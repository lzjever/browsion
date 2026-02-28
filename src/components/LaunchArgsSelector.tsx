import React, { useState } from 'react';

// Launch argument preset definitions
export interface ArgPreset {
  arg: string;
  description: string;
}

export interface ArgCategory {
  name: string;
  args: ArgPreset[];
}

// Common Chromium launch arguments organized by category
export const ARG_CATEGORIES: ArgCategory[] = [
  {
    name: 'Performance',
    args: [
      { arg: '--disable-gpu', description: 'Disable GPU hardware acceleration' },
      { arg: '--disable-dev-shm-usage', description: 'Use /tmp instead of /dev/shm (Docker/CI)' },
      { arg: '--disable-software-rasterizer', description: 'Disable software rasterizer' },
    ],
  },
  {
    name: 'Security',
    args: [
      { arg: '--no-sandbox', description: 'Disable sandbox (required in Docker/CI)' },
      { arg: '--disable-web-security', description: 'Disable same-origin policy (testing only)' },
      { arg: '--ignore-certificate-errors', description: 'Ignore SSL certificate errors' },
    ],
  },
  {
    name: 'Window',
    args: [
      { arg: '--start-maximized', description: 'Start browser maximized' },
      { arg: '--start-fullscreen', description: 'Start browser in fullscreen' },
      { arg: '--window-size=1920,1080', description: 'Set fixed window size 1920×1080' },
    ],
  },
  {
    name: 'Automation',
    args: [
      { arg: '--headless', description: 'Run in headless mode (no visible window)' },
      { arg: '--disable-images', description: 'Disable image loading (faster automation)' },
      { arg: '--disable-blink-features=AutomationControlled', description: 'Hide automation marker (removes navigator.webdriver)' },
    ],
  },
];

interface LaunchArgsSelectorProps {
  selectedArgs: string[];
  onArgsChange: (args: string[]) => void;
}

export const LaunchArgsSelector: React.FC<LaunchArgsSelectorProps> = ({
  selectedArgs,
  onArgsChange,
}) => {
  const [collapsed, setCollapsed] = useState(true);

  const toggleArg = (arg: string) => {
    if (selectedArgs.includes(arg)) {
      onArgsChange(selectedArgs.filter((a) => a !== arg));
    } else {
      onArgsChange([...selectedArgs, arg]);
    }
  };

  return (
    <div className="launch-args-selector">
      <button
        type="button"
        className="args-presets-toggle"
        onClick={() => setCollapsed((c) => !c)}
        aria-expanded={!collapsed}
      >
        <span className="args-presets-toggle-icon">{collapsed ? '▶' : '▼'}</span>
        <span>
          {collapsed
            ? `Presets (${selectedArgs.length} selected)`
            : 'Presets'}
        </span>
      </button>
      {!collapsed && (
        <div className="args-presets-content">
          {ARG_CATEGORIES.map((category) => (
            <div key={category.name} className="args-category-block">
              <div className="args-category-label">{category.name}</div>
              <ul className="args-option-list">
                {category.args.map((argPreset) => (
                  <li key={argPreset.arg} className="args-option-item">
                    <label className="args-option-label" title={argPreset.description}>
                      <input
                        type="checkbox"
                        checked={selectedArgs.includes(argPreset.arg)}
                        onChange={() => toggleArg(argPreset.arg)}
                      />
                      <span className="args-option-arg">{argPreset.arg}</span>
                    </label>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
