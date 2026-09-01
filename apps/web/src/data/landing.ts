export interface LandingProfile {
  id: string;
  title: string;
  description: string;
  tags: string[];
}

export interface LandingFeature {
  id?: string;
  number: string;
  title: string;
  description: string;
  command?: string;
  availability?: string;
}

export const profiles: LandingProfile[] = [
  {
    id: 'claude',
    title: 'Claude',
    description: 'A curated harness profile resolved as explicit, reviewable desired state.',
    tags: ['profile', 'selection']
  },
  {
    id: 'codex',
    title: 'Codex',
    description: 'A named harness profile with declared dependencies and provider validation.',
    tags: ['profile', 'providers']
  },
  {
    id: 'gemini',
    title: 'Gemini',
    description: 'A profile that contributes only the configuration and tools it declares.',
    tags: ['profile', 'mise']
  }
];

export const features: LandingFeature[] = [
  {
    number: '01',
    title: 'Validate the catalog',
    description: 'Confirm that every selected harness dependency has a valid, target-aware provider.',
    command: 'powermise registry validate'
  },
  {
    number: '02',
    title: 'Validate the selection',
    description: 'Check the requested profiles, inherited dependencies, tools, and machine target.',
    command: 'powermise selection validate --selection selections/engineering-agents.toml --target darwin'
  },
  {
    number: '03',
    title: 'Inspect the selection',
    description: 'Review the resolved configuration before compiling any generated output.',
    command: 'powermise selection show --selection selections/engineering-agents.toml --target darwin'
  },
  {
    id: 'selection-compile',
    number: '04',
    title: 'Compile configuration',
    description: 'Generate deterministic mise tools and bootstrap package configuration without mutating state.',
    command: 'powermise selection compile --selection selections/engineering-agents.toml --target darwin'
  }
];

export const safeguards = [
  ['Curated catalog', 'Versioned harness profiles with declared dependencies.'],
  ['Separate mise environment', 'Your global mise configuration remains user-owned.'],
  ['Explicit targets', 'Selections validate against macOS, Windows, or Linux.']
] as const;
