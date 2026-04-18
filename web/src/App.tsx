import { useState } from "react";

import AlertsPanel from "./components/AlertsPanel";
import AdminDashboard from "./components/AdminDashboard";
import BoxChatPanel from "./components/BoxChatPanel";
import CenterContent from "./components/CenterContent";
import ChatPatternRules from "./components/ChatPatternRules";
import EconomyPanel from "./components/EconomyPanel";
import ExtensionCatalogPanel from "./components/ExtensionCatalogPanel";
import GroupBuilder from "./components/GroupBuilder";
import LeftSidebar, { type ActiveView } from "./components/LeftSidebar";
import LootConfig from "./components/LootConfig";
import OperatorDashboard from "./components/OperatorDashboard";
import PlayerWatchPanel from "./components/PlayerWatchPanel";
import RightSidebar from "./components/RightSidebar";
import SayDetectionPanel from "./components/SayDetectionPanel";
import SoulPanel from "./components/SoulPanel";
import SpawnAlerts from "./components/SpawnAlerts";
import XAssistPanel from "./components/XAssistPanel";
import AdminDashboardPage from "./components/AdminDashboardPage";

function App() {
  const [activeView, setActiveView] = useState<ActiveView>("default");

  if (window.location.pathname === "/admin") {
    return <AdminDashboardPage />;
  }

  return (
    <div className="relative flex h-screen w-screen selection:bg-magentaglow selection:text-void">
      {/* Rotating sigil background */}
      <div className="bg-sigil" />
      {/* Radial gradient overlay */}
      <div className="pointer-events-none absolute inset-0 z-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-violet/20 via-void to-void" />
      {/* Main content */}
      <main className="relative z-10 flex h-full w-full gap-6 px-6 py-4">
        <LeftSidebar activeView={activeView} onNavigate={setActiveView} />
        {activeView === "engagements" ? (
          <OperatorDashboard />
        ) : activeView === "economy" ? (
          <EconomyPanel />
        ) : activeView === "formations" ? (
          <GroupBuilder />
        ) : activeView === "alerts" ? (
          <AlertsPanel />
        ) : activeView === "loot" ? (
          <LootConfig />
        ) : activeView === "soul" ? (
          <SoulPanel />
        ) : activeView === "spawns" ? (
          <SpawnAlerts />
        ) : activeView === "player_watch" ? (
          <PlayerWatchPanel />
        ) : activeView === "chat_pattern_rules" ? (
          <ChatPatternRules />
        ) : activeView === "extensions" ? (
          <ExtensionCatalogPanel />
        ) : activeView === "say" ? (
          <SayDetectionPanel />
        ) : activeView === "xassist" ? (
          <XAssistPanel />
        ) : activeView === "boxchat" ? (
          <BoxChatPanel />
        ) : activeView === "admin" ? (
          <AdminDashboard />
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
