import React from 'react';

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
      { arg: '--no-sandbox', description: 'Disable sandbox (Docker/CI required)' },
      { arg: '--disable-web-security', description: 'Disable same-origin policy (testing only)' },
      { arg: '--ignore-certificate-errors', description: 'Ignore SSL certificate errors' },
    ],
  },
  {
    name: 'Window',
    args: [
      { arg: '--start-maximized', description: 'Start browser maximized' },
      { arg: '--start-fullscreen', description: 'Start browser in fullscreen' },
    ],
  },
  {
    name: 'Network',
    args: [
      { arg: '--disable-background-networking', description: 'Disable background network requests' },
      { arg: '--disable-extensions', description: 'Disable browser extensions' },
    ],
  },
  {
    name: 'Automation',
    args: [
      { arg: '--disable-infobars', description: 'Hide Chrome infobars' },
      { arg: '--disable-blink-features=AutomationControlled', description: 'Hide automation detection' },
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
  const toggleArg = (arg: string) => {
    if (selectedArgs.includes(arg)) {
      onArgsChange(selectedArgs.filter((a) => a !== arg));
    } else {
      onArgsChange([...selectedArgs, arg]);
    }
  };

  return (
    <div className="launch-args-selector">
      {ARG_CATEGORIES.map((category) => (
        <div key={category.name} className="args-category">
          <div className="args-category-header">{category.name}</div>
          {category.args.map((argPreset) => (
            <label key={argPreset.arg} className="arg-item">
              <input
                type="checkbox"
                checked={selectedArgs.includes(argPreset.arg)}
                onChange={() => toggleArg(argPreset.arg)}
              />
              <div className="arg-info">
                <span className="arg-name">{argPreset.arg}</span>
                <div className="arg-desc">{argPreset.description}</div>
              </div>
            </label>
          ))}
        </div>
      ))}
    </div>
  );
};
