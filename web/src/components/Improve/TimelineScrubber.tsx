import React from "react";

interface TimelineScrubberProps {
  currentTime: number; // milliseconds
  maxTime: number; // milliseconds
  onTimeChange?: (time: number) => void;
}

export const TimelineScrubber: React.FC<TimelineScrubberProps> = ({
  currentTime,
  maxTime,
  onTimeChange,
}) => {
  const percentage = maxTime > 0 ? (currentTime / maxTime) * 100 : 0;

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newTime = (parseFloat(e.target.value) / 100) * maxTime;
    onTimeChange?.(newTime);
  };

  const formatTime = (ms: number): string => {
    const sec = Math.floor(ms / 1000);
    const min = Math.floor(sec / 60);
    const hr = Math.floor(min / 60);

    if (hr > 0)
      return `${hr}:${(min % 60).toString().padStart(2, "0")}:${(sec % 60).toString().padStart(2, "0")}`;
    return `${min}:${(sec % 60).toString().padStart(2, "0")}`;
  };

  return (
    <div className="flex items-center gap-3 px-4 py-3 border border-white/10 rounded bg-white/5">
      <span className="text-xs text-white/60 min-w-[45px]">
        {formatTime(currentTime)}
      </span>

      <input
        type="range"
        min="0"
        max="100"
        step="0.1"
        value={percentage}
        onChange={handleChange}
        className="flex-1 h-1 bg-white/10 rounded appearance-none cursor-pointer"
        style={{
          background: `linear-gradient(to right, rgb(59 130 246) 0%, rgb(59 130 246) ${percentage}%, rgb(255 255 255 / 0.1) ${percentage}%, rgb(255 255 255 / 0.1) 100%)`,
        }}
      />

      <span className="text-xs text-white/60 min-w-[45px] text-right">
        {formatTime(maxTime)}
      </span>
    </div>
  );
};

TimelineScrubber.displayName = "TimelineScrubber";
