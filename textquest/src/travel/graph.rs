use petgraph::algo::astar;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Directed;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PortCapability {
    WizardPort { destination: String },
    DruidPort { destination: String },
    Translocate { destination: String },
    Origin,
    Gate,
    Evac,
    Coth,
    MgbPort { destination: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CharacterClass {
    Warrior,
    Cleric,
    Druid,
    Enchanter,
    Wizard,
    Rogue,
    Bard,
    Other(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    pub name: String,
    pub class: CharacterClass,
    pub mana: u32,
    pub run_speed_mps: f64,
    pub capabilities: HashSet<PortCapability>,
}

impl Character {
    pub fn has_capability(&self, capability: &PortCapability) -> bool {
        self.capabilities.contains(capability)
    }

    pub fn signature(&self) -> CharacterSignature {
        let mut capabilities: Vec<_> = self.capabilities.iter().cloned().collect();
        capabilities.sort();
        CharacterSignature {
            class: self.class.clone(),
            mana: self.mana,
            run_speed_mps: self.run_speed_mps.to_bits(),
            capabilities,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Party {
    pub members: Vec<Character>,
}

impl Party {
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ZonePoint {
    pub id: String,
    pub zone: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub is_spire: bool,
    pub is_ring: bool,
}

impl ZonePoint {
    pub fn distance_meters(&self, other: &ZonePoint) -> f64 {
        let dx = f64::from(self.x - other.x);
        let dy = f64::from(self.y - other.y);
        let dz = f64::from(self.z - other.z);
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TravelEdgeKind {
    Walk,
    WizardSpire,
    DruidRing,
    Origin,
    Translocate,
    EvacGate,
    Coth,
    Mgb,
    Hedge,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum EdgeCost {
    WalkDistance { distance_meters: f64 },
    CastSeconds { seconds: f64, mana_cost: u32 },
    FixedSeconds { seconds: f64 },
}

impl EdgeCost {
    fn duration(&self, character: &Character) -> Option<Duration> {
        match self {
            Self::WalkDistance { distance_meters } => {
                if character.run_speed_mps <= 0.0 {
                    None
                } else {
                    Some(Duration::from_secs_f64(distance_meters / character.run_speed_mps))
                }
            }
            Self::CastSeconds { seconds, mana_cost } => {
                if character.mana < *mana_cost {
                    None
                } else {
                    Some(Duration::from_secs_f64(*seconds))
                }
            }
            Self::FixedSeconds { seconds } => Some(Duration::from_secs_f64(*seconds)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeTemplate {
    pub from: String,
    pub to: String,
    pub kind: TravelEdgeKind,
    pub cost: EdgeCost,
    pub requirement: EdgeRequirement,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EdgeRequirement {
    Always,
    CharacterHas(PortCapability),
    PartyHas(PortCapability),
    PartyHasMana {
        capability: PortCapability,
        minimum_mana: u32,
    },
    PartySizeAtLeast(usize),
    MaxDistanceMeters(f64),
}

impl EdgeRequirement {
    fn satisfied(
        &self,
        character: &Character,
        party: Option<&Party>,
        from: &ZonePoint,
        to: &ZonePoint,
    ) -> bool {
        match self {
            Self::Always => true,
            Self::CharacterHas(capability) => character.has_capability(capability),
            Self::PartyHas(capability) => party
                .map(|party| party.members.iter().any(|member| member.has_capability(capability)))
                .unwrap_or(false),
            Self::PartyHasMana {
                capability,
                minimum_mana,
            } => party
                .map(|party| {
                    party.members.iter().any(|member| {
                        member.has_capability(capability) && member.mana >= *minimum_mana
                    })
                })
                .unwrap_or(false),
            Self::PartySizeAtLeast(minimum) => party
                .map(|party| party.members.len() >= *minimum)
                .unwrap_or(false),
            Self::MaxDistanceMeters(max_distance) => from.distance_meters(to) <= *max_distance,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Edge {
    pub kind: TravelEdgeKind,
    pub cost: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RouteLeg {
    pub from: String,
    pub to: String,
    pub kind: TravelEdgeKind,
    pub cost: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Route {
    pub eta_min: Duration,
    pub route: Vec<RouteLeg>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CharacterSignature {
    class: CharacterClass,
    mana: u32,
    run_speed_mps: u64,
    capabilities: Vec<PortCapability>,
}

#[derive(Clone, Debug)]
pub struct TravelWorld {
    pub zone_points: Vec<ZonePoint>,
    pub edge_templates: Vec<EdgeTemplate>,
}

impl TravelWorld {
    pub fn new(zone_points: Vec<ZonePoint>, edge_templates: Vec<EdgeTemplate>) -> Self {
        Self {
            zone_points,
            edge_templates,
        }
    }
}

#[derive(Clone, Debug)]
struct CharacterGraph {
    graph: Graph<ZonePoint, Edge, Directed>,
    index_by_id: HashMap<String, NodeIndex>,
}

impl CharacterGraph {
    fn from_world(world: &TravelWorld, character: &Character, party: Option<&Party>) -> Self {
        let mut graph = Graph::<ZonePoint, Edge, Directed>::new();
        let mut index_by_id = HashMap::new();

        for point in &world.zone_points {
            let index = graph.add_node(point.clone());
            index_by_id.insert(point.id.clone(), index);
        }

        for template in &world.edge_templates {
            let Some(from_index) = index_by_id.get(&template.from).copied() else {
                continue;
            };
            let Some(to_index) = index_by_id.get(&template.to).copied() else {
                continue;
            };
            let from_point = graph.node_weight(from_index).cloned().unwrap();
            let to_point = graph.node_weight(to_index).cloned().unwrap();

            if !template
                .requirement
                .satisfied(character, party, &from_point, &to_point)
            {
                continue;
            }

            let Some(cost) = template.cost.duration(character) else {
                continue;
            };

            graph.add_edge(
                from_index,
                to_index,
                Edge {
                    kind: template.kind.clone(),
                    cost,
                },
            );
        }

        Self { graph, index_by_id }
    }

    fn index_for(&self, id: &str) -> Option<NodeIndex> {
        self.index_by_id.get(id).copied()
    }
}

pub trait TravelGraph {
    fn shortest_path(
        &self,
        from: ZonePoint,
        to: ZonePoint,
        character: &Character,
    ) -> Option<Route>;

    fn party_eta(&self, from: ZonePoint, to: ZonePoint, party: &Party) -> Duration;

    fn refresh_character(&self, character: &Character);
}

#[derive(Clone, Debug)]
pub struct TravelGraphService {
    world: Arc<TravelWorld>,
    cache: Arc<RwLock<HashMap<CharacterSignature, CharacterGraph>>>,
}

impl TravelGraphService {
    pub fn new(world: TravelWorld) -> Self {
        Self {
            world: Arc::new(world),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn graph_for_character(&self, character: &Character) -> CharacterGraph {
        let signature = character.signature();
        if let Some(graph) = self
            .cache
            .read()
            .expect("travel graph cache poisoned")
            .get(&signature)
            .cloned()
        {
            return graph;
        }

        let graph = CharacterGraph::from_world(&self.world, character, None);
        self.cache
            .write()
            .expect("travel graph cache poisoned")
            .insert(signature, graph.clone());
        graph
    }

    fn graph_for_party_member(&self, character: &Character, party: &Party) -> CharacterGraph {
        let mut graph = self.graph_for_character(character);
        let party_graph = CharacterGraph::from_world(&self.world, character, Some(party));

        for edge in party_graph.graph.edge_references() {
            let source = edge.source();
            let target = edge.target();
            graph.graph.add_edge(source, target, edge.weight().clone());
        }

        graph
    }

    fn route_from_graph(
        &self,
        graph: &CharacterGraph,
        from: &ZonePoint,
        to: &ZonePoint,
    ) -> Option<Route> {
        let start = graph.index_for(&from.id)?;
        let goal = graph.index_for(&to.id)?;

        let result = astar(
            &graph.graph,
            start,
            |finish| finish == goal,
            |edge| edge.weight().cost.as_micros() as u64,
            |_| 0u64,
        )?;

        let (total_micros, path) = result;
        let mut route = Vec::new();

        for pair in path.windows(2) {
            let from_index = pair[0];
            let to_index = pair[1];
            let from_point = graph.graph.node_weight(from_index)?.clone();
            let to_point = graph.graph.node_weight(to_index)?.clone();
            let edge = graph
                .graph
                .edges_connecting(from_index, to_index)
                .min_by_key(|edge| edge.weight().cost)?;
            let cost = edge.weight().cost;
            route.push(RouteLeg {
                from: from_point.id,
                to: to_point.id,
                kind: edge.weight().kind.clone(),
                cost,
            });
        }

        Some(Route {
            eta_min: Duration::from_micros(total_micros),
            route,
        })
    }
}

impl TravelGraph for TravelGraphService {
    fn shortest_path(
        &self,
        from: ZonePoint,
        to: ZonePoint,
        character: &Character,
    ) -> Option<Route> {
        let graph = self.graph_for_character(character);
        self.route_from_graph(&graph, &from, &to)
    }

    fn party_eta(&self, from: ZonePoint, to: ZonePoint, party: &Party) -> Duration {
        if party.is_empty() {
            return Duration::from_secs(0);
        }

        party
            .members
            .iter()
            .filter_map(|member| {
                let graph = self.graph_for_party_member(member, party);
                self.route_from_graph(&graph, &from, &to).map(|route| route.eta_min)
            })
            .max()
            .unwrap_or_else(|| Duration::from_secs(u64::MAX / 2))
    }

    fn refresh_character(&self, character: &Character) {
        let signature = character.signature();
        let graph = CharacterGraph::from_world(&self.world, character, None);
        self.cache
            .write()
            .expect("travel graph cache poisoned")
            .insert(signature, graph);
    }
}

fn point(id: &str, zone: &str, x: i32, y: i32, z: i32) -> ZonePoint {
    ZonePoint {
        id: id.to_string(),
        zone: zone.to_string(),
        x,
        y,
        z,
        is_spire: false,
        is_ring: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn druid() -> Character {
        let mut capabilities = HashSet::new();
        capabilities.insert(PortCapability::DruidPort {
            destination: "Plane of Fire".to_string(),
        });
        Character {
            name: "Sorn".to_string(),
            class: CharacterClass::Druid,
            mana: 2000,
            run_speed_mps: 5.0,
            capabilities,
        }
    }

    fn wizard() -> Character {
        let mut capabilities = HashSet::new();
        capabilities.insert(PortCapability::WizardPort {
            destination: "Plane of Knowledge".to_string(),
        });
        Character {
            name: "Elya".to_string(),
            class: CharacterClass::Wizard,
            mana: 500,
            run_speed_mps: 5.5,
            capabilities,
        }
    }

    fn cother(mana: u32) -> Character {
        let mut capabilities = HashSet::new();
        capabilities.insert(PortCapability::Coth);
        Character {
            name: "Puller".to_string(),
            class: CharacterClass::Cleric,
            mana,
            run_speed_mps: 5.0,
            capabilities,
        }
    }

    fn world_with_poif_hedge() -> TravelWorld {
        TravelWorld::new(
            vec![
                point("pop", "Plane of Power", 0, 0, 0),
                point("pof", "Plane of Fire", 1, 0, 0),
            ],
            vec![EdgeTemplate {
                from: "pop".to_string(),
                to: "pof".to_string(),
                kind: TravelEdgeKind::Hedge,
                cost: EdgeCost::FixedSeconds { seconds: 30.0 },
                requirement: EdgeRequirement::CharacterHas(PortCapability::DruidPort {
                    destination: "Plane of Fire".to_string(),
                }),
            }],
        )
    }

    #[test]
    fn shortest_path_requires_known_port() {
        let service = TravelGraphService::new(world_with_poif_hedge());
        let from = point("pop", "Plane of Power", 0, 0, 0);
        let to = point("pof", "Plane of Fire", 1, 0, 0);

        assert!(service.shortest_path(from.clone(), to.clone(), &wizard()).is_none());

        let route = service
            .shortest_path(from, to, &druid())
            .expect("druid hedge should exist");
        assert_eq!(route.eta_min, Duration::from_secs(30));
        assert_eq!(route.route.len(), 1);
        assert_eq!(route.route[0].kind, TravelEdgeKind::Hedge);
    }

    #[test]
    fn party_eta_uses_worst_case_member() {
        let world = TravelWorld::new(
            vec![
                point("start", "Commonlands", 0, 0, 0),
                point("end", "Commonlands", 100, 0, 0),
            ],
            vec![
                EdgeTemplate {
                    from: "start".to_string(),
                    to: "end".to_string(),
                    kind: TravelEdgeKind::Walk,
                    cost: EdgeCost::WalkDistance {
                        distance_meters: 100.0,
                    },
                    requirement: EdgeRequirement::Always,
                },
                EdgeTemplate {
                    from: "start".to_string(),
                    to: "end".to_string(),
                    kind: TravelEdgeKind::WizardSpire,
                    cost: EdgeCost::FixedSeconds { seconds: 10.0 },
                    requirement: EdgeRequirement::CharacterHas(PortCapability::WizardPort {
                        destination: "Commonlands".to_string(),
                    }),
                },
            ],
        );
        let service = TravelGraphService::new(world);
        let party = Party {
            members: vec![
                Character {
                    name: "Fast".to_string(),
                    class: CharacterClass::Wizard,
                    mana: 1000,
                    run_speed_mps: 10.0,
                    capabilities: {
                        let mut caps = HashSet::new();
                        caps.insert(PortCapability::WizardPort {
                            destination: "Commonlands".to_string(),
                        });
                        caps
                    },
                },
                Character {
                    name: "Slow".to_string(),
                    class: CharacterClass::Warrior,
                    mana: 0,
                    run_speed_mps: 2.0,
                    capabilities: HashSet::new(),
                },
            ],
        };

        let eta = service.party_eta(
            point("start", "Commonlands", 0, 0, 0),
            point("end", "Commonlands", 100, 0, 0),
            &party,
        );

        assert!(eta >= Duration::from_secs(10));
    }

    #[test]
    fn coth_requires_party_caster_and_mana() {
        let world = TravelWorld::new(
            vec![
                point("pull", "Plane of Knowledge", 0, 0, 0),
                point("camp", "Sebilis", 10, 0, 0),
            ],
            vec![EdgeTemplate {
                from: "pull".to_string(),
                to: "camp".to_string(),
                kind: TravelEdgeKind::Coth,
                cost: EdgeCost::CastSeconds {
                    seconds: 6.5,
                    mana_cost: 1200,
                },
                requirement: EdgeRequirement::PartyHasMana {
                    capability: PortCapability::Coth,
                    minimum_mana: 1200,
                },
            }],
        );
        let service = TravelGraphService::new(world);

        let party_without_caster = Party {
            members: vec![Character {
                name: "Tank".to_string(),
                class: CharacterClass::Warrior,
                mana: 0,
                run_speed_mps: 4.0,
                capabilities: HashSet::new(),
            }],
        };
        let eta = service.party_eta(
            point("pull", "Plane of Knowledge", 0, 0, 0),
            point("camp", "Sebilis", 10, 0, 0),
            &party_without_caster,
        );
        assert_eq!(eta, Duration::from_secs(u64::MAX / 2));

        let party_with_caster = Party {
            members: vec![
                cother(1500),
                Character {
                    name: "Tank".to_string(),
                    class: CharacterClass::Warrior,
                    mana: 0,
                    run_speed_mps: 4.0,
                    capabilities: HashSet::new(),
                },
            ],
        };
        let eta = service.party_eta(
            point("pull", "Plane of Knowledge", 0, 0, 0),
            point("camp", "Sebilis", 10, 0, 0),
            &party_with_caster,
        );
        assert_eq!(eta, Duration::from_secs_f64(6.5));
    }

    #[test]
    fn dijkstra_stays_fast_for_150_nodes() {
        let mut zone_points = Vec::new();
        let mut edges = Vec::new();

        for index in 0..150 {
            zone_points.push(point(
                &format!("z{index}"),
                "LiveEra",
                index as i32,
                0,
                0,
            ));
            if index > 0 {
                edges.push(EdgeTemplate {
                    from: format!("z{}", index - 1),
                    to: format!("z{index}"),
                    kind: TravelEdgeKind::Walk,
                    cost: EdgeCost::WalkDistance {
                        distance_meters: 1.0,
                    },
                    requirement: EdgeRequirement::Always,
                });
            }
        }

        let service = TravelGraphService::new(TravelWorld::new(zone_points, edges));
        let character = Character {
            name: "Runner".to_string(),
            class: CharacterClass::Rogue,
            mana: 0,
            run_speed_mps: 5.0,
            capabilities: HashSet::new(),
        };
        let from = point("z0", "LiveEra", 0, 0, 0);
        let to = point("z149", "LiveEra", 149, 0, 0);

        let start = Instant::now();
        let route = service
            .shortest_path(from, to, &character)
            .expect("simple chain should have a path");
        let elapsed = start.elapsed();

        assert_eq!(route.route.len(), 149);
        assert!(elapsed < Duration::from_millis(50), "elapsed: {elapsed:?}");
    }
}
