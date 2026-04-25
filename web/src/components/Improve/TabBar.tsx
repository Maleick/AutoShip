import React from "react";

type TabType = "dps" | "healing" | "cc" | "deaths" | "resources" | "pulls";

interface TabBarProps {
  activeTab: TabType;
  onTabChange: (tab: TabType) => void;
  availableTabs?: TabType[];
}

const tabLabels: Record<TabType, string> = {
  dps: "DPS",
  healing: "Healing",
  cc: "CC",
  deaths: "Deaths",
  resources: "Resources",
  pulls: "Pulls",
};

export const TabBar: React.FC<TabBarProps> = ({
  activeTab,
  onTabChange,
  availableTabs = ["dps", "healing", "cc", "deaths", "resources", "pulls"],
}) => {
  return (
    <div className="flex gap-1 border-b border-white/10">
      {availableTabs.map((tab) => (
        <button
          key={tab}
          onClick={() => onTabChange(tab)}
          className={`px-4 py-2 text-sm font-archaic transition-all border-b-2 ${
            activeTab === tab
              ? "border-white text-white"
              : "border-transparent text-white/60 hover:text-white/80"
          }`}
        >
          {tabLabels[tab]}
        </button>
      ))}
    </div>
  );
};

TabBar.displayName = "TabBar";
