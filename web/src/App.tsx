import { useState } from "react";
import LeftSidebar, { type ActiveView } from "./components/LeftSidebar";
import CenterContent from "./components/CenterContent";
import RightSidebar from "./components/RightSidebar";
import GroupBuilder from "./components/GroupBuilder";
import LootConfig from "./components/LootConfig";
import SayDetectionPanel from "./components/SayDetectionPanel";
import SoulPanel from "./components/SoulPanel";
import AlertsPanel from "./components/AlertsPanel";
import SpawnAlerts from "./components/SpawnAlerts";
import ChatPatternRules from "./components/ChatPatternRules";

function App() {
  const [activeView, setActiveView] = useState<ActiveView>("engagements");

  return (
    <div className="flex h-screen w-screen selection:bg-magentaglow selection:text-void relative">
      {/* Rotating sigil background */}
      <div className="bg-sigil" />
      {/* Radial gradient overlay */}
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-violet/20 via-void to-void z-0 pointer-events-none" />
      {/* Main content */}
      <main className="relative z-10 w-full h-full flex px-6 py-4 gap-6">
        <LeftSidebar activeView={activeView} onNavigate={setActiveView} />
        {activeView === "formations" ? (
          <GroupBuilder />
        ) : activeView === "alerts" ? (
          <AlertsPanel />
        ) : activeView === "loot" ? (
          <LootConfig />
        ) : activeView === "soul" ? (
          <SoulPanel />
        ) : activeView === "spawns" ? (
          <SpawnAlerts />
        ) : activeView === "chat_pattern_rules" ? (
          <ChatPatternRules />
        ) : activeView === "say" ? (
          <SayDetectionPanel />
        ) : (
          <>
            <CenterContent />
            <RightSidebar />
          </>
        )}
      </main>
    </div>
  );
}

export default App;
