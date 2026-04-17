import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Bell,
  CaretDown,
  ChatCircle,
  Check,
  Clock,
  Lightning,
  PencilSimple,
  Plus,
  Trash,
  X,
} from "@phosphor-icons/react";

interface ActionDto {
  actionType: string;
  payload: string;
}

interface ChatPatternRule {
  id: string;
  name: string;
  pattern: string;
  patternType: "literal" | "regex";
  channels: string[];
  action: ActionDto;
  priority: number;
  enabled: boolean;
  cooldownSecs: number;
  fireCount: number;
}

interface RuleStats {
  totalRules: number;
  enabledRules: number;
  totalFires: number;
}

interface CreateRuleRequest {
  name: string;
  pattern: string;
  patternType: string;
  channels: string[];
  action: ActionDto;
  priority: number;
  enabled: boolean;
  cooldownSecs: number;
}

const CHAT_CHANNELS = [
  { id: "say", label: "Say" },
  { id: "tell", label: "Tell" },
  { id: "group", label: "Group" },
  { id: "guild", label: "Guild" },
  { id: "raid", label: "Raid" },
  { id: "shout", label: "Shout" },
  { id: "ooc", label: "OOC" },
  { id: "auction", label: "Auction" },
];

const ACTION_TYPES = [
  { id: "execute_command", label: "Execute Command" },
  { id: "send_ipc", label: "Send IPC" },
  { id: "trigger_alert", label: "Trigger Alert" },
];

function Section({
  icon,
  title,
  subtitle,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <section className="bg-violet/20 border border-white/10 p-5">
      <div className="flex items-center gap-3 mb-4">
        <div className="w-8 h-8 border border-magentaglow/40 bg-magentadark/15 text-magentaglow flex items-center justify-center">
          {icon}
        </div>
        <div>
          <h3 className="font-archaic text-base text-white">{title}</h3>
          <p className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
            {subtitle}
          </p>
        </div>
      </div>
      {children}
    </section>
  );
}

function RuleEditor({
  rule,
  onSave,
  onCancel,
}: {
  rule?: ChatPatternRule;
  onSave: (r: CreateRuleRequest) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(rule?.name || "");
  const [pattern, setPattern] = useState(rule?.pattern || "");
  const [patternType, setPatternType] = useState<"literal" | "regex">(
    (rule?.patternType as "literal" | "regex") || "literal"
  );
  const [channels, setChannels] = useState<string[]>(rule?.channels || []);
  const [actionType, setActionType] = useState(rule?.action.actionType || "execute_command");
  const [actionPayload, setActionPayload] = useState(rule?.action.payload || "");
  const [priority, setPriority] = useState(rule?.priority || 100);
  const [cooldown, setCooldown] = useState(rule?.cooldownSecs || 0);
  const [enabled, setEnabled] = useState(rule?.enabled ?? true);

  const toggleChannel = (channelId: string) => {
    setChannels((prev) =>
      prev.includes(channelId)
        ? prev.filter((c) => c !== channelId)
        : [...prev, channelId]
    );
  };

  const handleSave = () => {
    if (!name.trim() || !pattern.trim() || !actionPayload.trim()) return;
    onSave({
      name: name.trim(),
      pattern: pattern.trim(),
      patternType,
      channels,
      action: { actionType, payload: actionPayload.trim() },
      priority,
      enabled,
      cooldownSecs: cooldown,
    });
  };

  return (
    <div className="fixed inset-0 bg-black/60 z-50 flex items-center justify-center p-8">
      <div className="bg-violet/30 border border-magentaglow/30 w-full max-w-2xl max-h-[90vh] overflow-y-auto">
        <div className="p-6 border-b border-white/10 flex items-center justify-between">
          <h2 className="font-archaic text-xl text-white">
            {rule ? "Edit Rule" : "Create Rule"}
          </h2>
          <button onClick={onCancel} className="text-white/50 hover:text-white">
            <X size={20} />
          </button>
        </div>

        <div className="p-6 space-y-6">
          <div>
            <label className="block text-xs uppercase tracking-widest text-white/60 font-rune mb-2">
              Rule Name
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g., Rez Response"
              className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs uppercase tracking-widest text-white/60 font-rune mb-2">
                Pattern Type
              </label>
              <select
                value={patternType}
                onChange={(e) => setPatternType(e.target.value as "literal" | "regex")}
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
              >
                <option value="literal">Literal (substring match)</option>
                <option value="regex">Regex (full pattern)</option>
              </select>
            </div>
            <div>
              <label className="block text-xs uppercase tracking-widest text-white/60 font-rune mb-2">
                Priority
              </label>
              <input
                type="number"
                value={priority}
                onChange={(e) => setPriority(parseInt(e.target.value) || 100)}
                min={1}
                max={9999}
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs uppercase tracking-widest text-white/60 font-rune mb-2">
              Pattern
            </label>
            <input
              type="text"
              value={pattern}
              onChange={(e) => setPattern(e.target.value)}
              placeholder={patternType === "regex" ? "e.g., \\d+ gold|You receive.*" : "e.g., rez me or need a"}
              className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
            />
            <p className="text-[10px] text-white/40 mt-1">
              {patternType === "literal"
                ? "Matches if message contains this text (case-insensitive)"
                : "Full regex pattern (case-insensitive by default)"}
            </p>
          </div>

          <div>
            <label className="block text-xs uppercase tracking-widest text-white/60 font-rune mb-2">
              Chat Channels
            </label>
            <div className="flex flex-wrap gap-2">
              {CHAT_CHANNELS.map((channel) => (
                <button
                  key={channel.id}
                  onClick={() => toggleChannel(channel.id)}
                  className={`px-3 py-1.5 border text-xs font-rune transition-colors ${
                    channels.includes(channel.id)
                      ? "border-magentaglow/50 bg-magentadark/20 text-magentaglow"
                      : "border-white/20 bg-void/40 text-white/50 hover:border-white/40"
                  }`}
                >
                  {channel.label}
                </button>
              ))}
            </div>
            <p className="text-[10px] text-white/40 mt-1">
              Empty = match all channels
            </p>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs uppercase tracking-widest text-white/60 font-rune mb-2">
                Action Type
              </label>
              <select
                value={actionType}
                onChange={(e) => setActionType(e.target.value)}
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
              >
                {ACTION_TYPES.map((at) => (
                  <option key={at.id} value={at.id}>
                    {at.label}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-xs uppercase tracking-widest text-white/60 font-rune mb-2">
                Cooldown (seconds)
              </label>
              <input
                type="number"
                value={cooldown}
                onChange={(e) => setCooldown(parseInt(e.target.value) || 0)}
                min={0}
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs uppercase tracking-widest text-white/60 font-rune mb-2">
              Action Payload
            </label>
            <input
              type="text"
              value={actionPayload}
              onChange={(e) => setActionPayload(e.target.value)}
              placeholder={
                actionType === "execute_command"
                  ? "e.g., /tell MyHealer I need a rez"
                  : actionType === "send_ipc"
                  ? "e.g., alert:rez_request"
                  : "e.g., Critical Alert"
              }
              className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
            />
          </div>

          <div>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={enabled}
                onChange={(e) => setEnabled(e.target.checked)}
                className="w-4 h-4 accent-magentaglow"
              />
              <span className="text-sm text-white font-rune">Rule Enabled</span>
            </label>
          </div>
        </div>

        <div className="p-6 border-t border-white/10 flex justify-end gap-3">
          <button
            onClick={onCancel}
            className="px-4 py-2 border border-white/20 text-white/70 hover:text-white text-xs uppercase tracking-wider font-rune"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={!name.trim() || !pattern.trim() || !actionPayload.trim()}
            className="px-4 py-2 border border-magentaglow/50 text-magentaglow bg-magentadark/20 hover:bg-magentadark/40 text-xs uppercase tracking-wider font-rune disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {rule ? "Save Changes" : "Create Rule"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default function ChatPatternRules() {
  const [rules, setRules] = useState<ChatPatternRule[]>([]);
  const [stats, setStats] = useState<RuleStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [showEditor, setShowEditor] = useState(false);
  const [editingRule, setEditingRule] = useState<ChatPatternRule | undefined>(undefined);
  const [search, setSearch] = useState("");
  const [showEnabledOnly, setShowEnabledOnly] = useState(false);
  const [sortBy, setSortBy] = useState<"priority" | "name" | "fires">("priority");

  const loadRules = useCallback(async () => {
    try {
      const [rulesRes, statsRes] = await Promise.all([
        fetch("/api/chat-pattern-rules"),
        fetch("/api/chat-pattern-rules/stats"),
      ]);
      if (rulesRes.ok) {
        const data = await rulesRes.json();
        setRules(data);
      }
      if (statsRes.ok) {
        const data = await statsRes.json();
        setStats(data);
      }
    } catch (err) {
      console.error("Failed to load rules:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadRules();
  }, [loadRules]);

  const filteredRules = useMemo(() => {
    let result = [...rules];
    if (search.trim()) {
      const needle = search.trim().toLowerCase();
      result = result.filter(
        (r) =>
          r.name.toLowerCase().includes(needle) ||
          r.pattern.toLowerCase().includes(needle)
      );
    }
    if (showEnabledOnly) {
      result = result.filter((r) => r.enabled);
    }
    result.sort((a, b) => {
      if (sortBy === "priority") return a.priority - b.priority;
      if (sortBy === "name") return a.name.localeCompare(b.name);
      return b.fireCount - a.fireCount;
    });
    return result;
  }, [rules, search, showEnabledOnly, sortBy]);

  const handleSave = async (request: CreateRuleRequest) => {
    const method = editingRule ? "PUT" : "POST";
    const url = editingRule
      ? `/api/chat-pattern-rules/${editingRule.id}`
      : "/api/chat-pattern-rules";

    const res = await fetch(url, {
      method,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });

    if (res.ok) {
      setShowEditor(false);
      setEditingRule(undefined);
      loadRules();
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Delete this rule?")) return;
    const res = await fetch(`/api/chat-pattern-rules/${id}`, { method: "DELETE" });
    if (res.ok) {
      loadRules();
    }
  };

  const handleToggle = async (id: string) => {
    const res = await fetch(`/api/chat-pattern-rules/${id}/toggle`, { method: "PUT" });
    if (res.ok) {
      loadRules();
    }
  };

  const formatAction = (action: ActionDto) => {
    switch (action.actionType) {
      case "execute_command":
        return <span className="text-cyan-400">{action.payload}</span>;
      case "send_ipc":
        return <span className="text-magentaglow">{action.payload}</span>;
      case "trigger_alert":
        return <span className="text-yellow-400">{action.payload}</span>;
      default:
        return action.payload;
    }
  };

  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[700px]">
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md shrink-0">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-magentadark flex items-center justify-center bg-void">
            <ChatCircle weight="fill" className="text-magentaglow" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Chat Pattern Rules
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              MQ2Events / MQ2React parity · Pattern matching · Automated responses
            </p>
          </div>
        </div>
        <button
          onClick={() => {
            setEditingRule(undefined);
            setShowEditor(true);
          }}
          className="px-4 py-2 border border-magentaglow/50 text-magentaglow bg-magentadark/20 hover:bg-magentadark/40 text-xs uppercase tracking-wider font-rune flex items-center gap-2"
        >
          <Plus size={14} />
          New Rule
        </button>
      </header>

      <div className="flex-1 overflow-y-auto p-6 scroll-smooth space-y-6">
        {stats && (
          <div className="grid grid-cols-4 gap-4">
            <div className="bg-void/60 border border-white/10 p-4 flex flex-col gap-1">
              <span className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
                Total Rules
              </span>
              <span className="font-rune text-2xl text-white">{stats.totalRules}</span>
            </div>
            <div className="bg-void/60 border border-white/10 p-4 flex flex-col gap-1">
              <span className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
                Enabled
              </span>
              <span className="font-rune text-2xl text-magentaglow">
                {stats.enabledRules}
              </span>
            </div>
            <div className="bg-void/60 border border-white/10 p-4 flex flex-col gap-1">
              <span className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
                Total Fires
              </span>
              <span className="font-rune text-2xl text-cyan-400">
                {stats.totalFires.toLocaleString()}
              </span>
            </div>
            <div className="bg-void/60 border border-white/10 p-4 flex flex-col gap-1">
              <span className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
                Active Sources
              </span>
              <span className="font-rune text-2xl text-spectral">
                {CHAT_CHANNELS.length}
              </span>
            </div>
          </div>
        )}

        <Section
          icon={<Lightning size={15} />}
          title="Rule Engine"
          subtitle="Pattern-based chat event triggers"
        >
          <div className="flex items-center gap-4 mb-4">
            <div className="relative flex-1">
              <input
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search rules..."
                className="w-full bg-void border border-white/20 text-white text-sm pl-10 pr-3 py-2 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/20"
              />
              <ChatCircle
                size={15}
                className="absolute left-3 top-1/2 -translate-y-1/2 text-white/30"
              />
            </div>
            <select
              value={sortBy}
              onChange={(e) => setSortBy(e.target.value as "priority" | "name" | "fires")}
              className="bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
            >
              <option value="priority">Sort by Priority</option>
              <option value="name">Sort by Name</option>
              <option value="fires">Sort by Fires</option>
            </select>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={showEnabledOnly}
                onChange={(e) => setShowEnabledOnly(e.target.checked)}
                className="w-4 h-4 accent-magentaglow"
              />
              <span className="text-sm text-white font-rune">Enabled Only</span>
            </label>
          </div>

          {loading ? (
            <div className="flex items-center justify-center h-32 text-white/40 font-archaic">
              Loading rules...
            </div>
          ) : filteredRules.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-32 gap-2 text-white/40 font-archaic">
              <ChatCircle size={32} />
              {rules.length === 0
                ? "No rules configured. Create your first rule."
                : "No rules match your filters."}
            </div>
          ) : (
            <div className="space-y-2">
              {filteredRules.map((rule) => (
                <div
                  key={rule.id}
                  className={`grid grid-cols-[1.5fr_2fr_1.5fr_0.8fr_0.8fr_auto] gap-3 border bg-void/40 p-3 items-center ${
                    rule.enabled
                      ? "border-magentaglow/20"
                      : "border-white/10 opacity-60"
                  }`}
                >
                  <div className="flex items-center gap-2">
                    <span className="text-white text-sm font-rune">{rule.name}</span>
                    <span className="text-[10px] px-1.5 py-0.5 border font-rune text-white/40">
                      #{rule.priority}
                    </span>
                  </div>
                  <div className="flex flex-col gap-1">
                    <span className="text-white/70 text-xs font-mono truncate">
                      {rule.pattern}
                    </span>
                    <span className="text-[10px] text-white/30 font-rune">
                      {rule.patternType === "regex" ? "Regex" : "Literal"}
                      {rule.channels.length > 0 && ` · ${rule.channels.join(", ")}`}
                      {rule.channels.length === 0 && " · All channels"}
                    </span>
                  </div>
                  <div className="flex items-center gap-1 text-xs">
                    {rule.action.actionType === "execute_command" && (
                      <Lightning size={12} className="text-cyan-400" />
                    )}
                    {rule.action.actionType === "send_ipc" && (
                      <ChatCircle size={12} className="text-magentaglow" />
                    )}
                    {rule.action.actionType === "trigger_alert" && (
                      <Bell size={12} className="text-yellow-400" />
                    )}
                    <span className="truncate">{formatAction(rule.action)}</span>
                  </div>
                  <div className="flex items-center gap-1 text-white/50 text-xs font-rune">
                    <Clock size={12} />
                    {rule.cooldownSecs > 0 ? `${rule.cooldownSecs}s cd` : "No cd"}
                  </div>
                  <div className="flex items-center gap-1 text-white/50 text-xs font-rune">
                    <Check size={12} />
                    {rule.fireCount} fires
                  </div>
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => handleToggle(rule.id)}
                      className={`w-6 h-6 border flex items-center justify-center text-xs ${
                        rule.enabled
                          ? "border-green-500/50 text-green-400 hover:bg-green-500/10"
                          : "border-white/20 text-white/30 hover:bg-white/5"
                      }`}
                    >
                      {rule.enabled ? "On" : "Off"}
                    </button>
                    <button
                      onClick={() => {
                        setEditingRule(rule);
                        setShowEditor(true);
                      }}
                      className="w-6 h-6 border border-white/20 text-white/50 hover:text-white hover:border-white/40 flex items-center justify-center"
                    >
                      <PencilSimple size={12} />
                    </button>
                    <button
                      onClick={() => handleDelete(rule.id)}
                      className="w-6 h-6 border border-white/20 text-white/30 hover:text-red-400 hover:border-red-400/50 flex items-center justify-center"
                    >
                      <Trash size={12} />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </Section>

        <Section
          icon={<ChatCircle size={15} />}
          title="Event Sources"
          subtitle="Supported chat channels"
        >
          <div className="flex flex-wrap gap-2">
            {CHAT_CHANNELS.map((channel) => (
              <div
                key={channel.id}
                className="px-3 py-1.5 border border-magentaglow/30 bg-magentadark/10 text-magentaglow text-xs font-rune"
              >
                {channel.label}
              </div>
            ))}
            <div className="px-3 py-1.5 border border-white/20 bg-void/40 text-white/50 text-xs font-rune">
              System (future)
            </div>
          </div>
        </Section>

        <Section
          icon={<Bell size={15} />}
          title="Action Types"
          subtitle="Available trigger actions"
        >
          <div className="space-y-2">
            <div className="flex items-start gap-3">
              <Lightning size={16} className="text-cyan-400 mt-0.5" />
              <div>
                <span className="text-white text-sm font-rune">Execute Command</span>
                <p className="text-[10px] text-white/40">
                  Runs an EQ slash command when the pattern matches
                </p>
              </div>
            </div>
            <div className="flex items-start gap-3">
              <ChatCircle size={16} className="text-magentaglow mt-0.5" />
              <div>
                <span className="text-white text-sm font-rune">Send IPC</span>
                <p className="text-[10px] text-white/40">
                  Sends an inter-process command to other sessions
                </p>
              </div>
            </div>
            <div className="flex items-start gap-3">
              <Bell size={16} className="text-yellow-400 mt-0.5" />
              <div>
                <span className="text-white text-sm font-rune">Trigger Alert</span>
                <p className="text-[10px] text-white/40">
                  Fires a sound or visual alert notification
                </p>
              </div>
            </div>
          </div>
        </Section>
      </div>

      {showEditor && (
        <RuleEditor
          rule={editingRule}
          onSave={handleSave}
          onCancel={() => {
            setShowEditor(false);
            setEditingRule(undefined);
          }}
        />
      )}
    </section>
  );
}
