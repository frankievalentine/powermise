import type { Platform } from "../api";

// Hand-curated Discover data. Popularity comes from popularity.json
// (scripts/discover-popularity.py); this file adds what a ranking can't:
// categories, friendlier descriptions, everyday system utilities mise
// installs as packages, and popular tools from other ecosystems.

export interface DiscoverCategory {
  id: string;
  title: string;
  /** mise registry names, in no particular order (Discover sorts by popularity). */
  tools: string[];
  /** Shown only on these platforms. */
  platforms?: Platform[];
}

export const CATEGORIES: DiscoverCategory[] = [
  {
    id: "languages",
    title: "Languages & runtimes",
    tools: ["node", "python", "go", "rust", "java", "ruby", "php", "bun", "deno", "dart", "flutter", "elixir", "erlang", "zig", "kotlin", "scala", "swift", "dotnet", "lua", "perl", "julia", "crystal", "nim", "opam", "ghc", "stack", "clojure", "groovy", "graalvm"],
  },
  {
    id: "package-managers",
    title: "Package & build tools",
    tools: ["uv", "pnpm", "yarn", "pipx", "poetry", "maven", "gradle", "cmake", "ninja", "ccache", "bazel", "just", "task", "cocoapods", "protobuf", "make", "meson", "sbt", "leiningen", "conan"],
  },
  {
    id: "cloud",
    title: "Cloud & DevOps",
    tools: ["aws-cli", "gcloud", "azure", "terraform", "opentofu", "pulumi", "ansible", "flyctl", "vercel", "supabase", "cloudflared", "doctl", "heroku", "packer", "vault", "consul", "nomad", "act", "actionlint", "gh", "glab", "stripe", "firebase", "railway", "wrangler"],
  },
  {
    id: "containers",
    title: "Containers & Kubernetes",
    tools: ["docker-cli", "docker-compose", "podman", "colima", "kubectl", "helm", "k9s", "kustomize", "kind", "minikube", "k3d", "argocd", "flux2", "kubeconform", "stern", "kubectx", "skaffold", "tilt", "lazydocker", "dive", "docker-slim"],
  },
  {
    id: "databases",
    title: "Databases",
    tools: ["postgres", "mysql", "redis", "sqlite", "mongodb", "duckdb", "usql", "atlas", "flyway", "dbmate", "sqlc"],
  },
  {
    id: "essentials",
    title: "Command-line essentials",
    tools: ["jq", "yq", "ripgrep", "fd", "fzf", "bat", "eza", "zoxide", "delta", "coreutils", "htmlq", "jless", "fx", "sd", "dust", "duf", "tealdeer", "hyperfine", "watchexec", "rclone", "mkcert", "git-lfs", "lazygit", "gum", "glow", "xh", "curlie", "age", "sops", "direnv", "shellcheck", "shfmt", "atuin", "fastfetch"],
  },
  {
    id: "editors",
    title: "Editors & terminal",
    tools: ["neovim", "vim", "helix", "tmux", "zellij", "starship", "btop", "bottom", "yazi", "lazygit"],
  },
  {
    id: "ai",
    title: "AI & ML",
    tools: ["ollama", "llama.cpp", "claude", "codex", "gemini-cli", "opencode", "crush"],
  },
  {
    id: "media",
    title: "Media & documents",
    tools: ["ffmpeg", "imagemagick", "yt-dlp", "pandoc", "poppler", "exiftool", "typst", "hugo", "mdbook", "d2", "vhs"],
  },
  {
    id: "mobile",
    title: "Mobile & Apple",
    platforms: ["darwin"],
    tools: ["xcbeautify", "swiftlint", "swiftformat", "xcodegen", "tuist", "cocoapods", "mint", "periphery", "sourcery", "xcodes"],
  },
];

/** Plainer descriptions for popular tools whose registry text is thin. */
export const DESCRIPTIONS: Record<string, string> = {
  gh: "GitHub on the command line: pull requests, issues, releases, and Actions.",
  node: "Node.js, the JavaScript runtime used by most web tooling.",
  "aws-cli": "Amazon Web Services from the terminal.",
  uv: "A very fast Python package and project manager.",
  ffmpeg: "Convert, cut, and stream audio and video.",
  cmake: "Builds C and C++ projects across platforms.",
  go: "The Go programming language.",
  python: "The Python programming language.",
  coreutils: "GNU versions of core commands like ls, cp, and date.",
  imagemagick: "Convert and edit images from the command line.",
  jq: "Filter and reshape JSON.",
  "gemini-cli": "Google's Gemini AI agent in your terminal.",
  "docker-cli": "The Docker command for building and running containers.",
  pipx: "Install Python command-line apps in their own environments.",
  ollama: "Run large language models on your own computer.",
  "yt-dlp": "Download video and audio from YouTube and many other sites.",
  yq: "Like jq, for YAML (and JSON, XML, TOML).",
  pnpm: "A fast, disk-efficient JavaScript package manager.",
  tmux: "Split one terminal into many, and keep sessions running.",
  fzf: "Fuzzy-find files, history, and anything else as you type.",
  ripgrep: "Search the contents of files, very fast.",
  neovim: "A modern, extensible Vim.",
  helm: "Install and manage apps on Kubernetes.",
  kubectl: "Control Kubernetes clusters.",
  protobuf: "Protocol Buffers compiler for gRPC and data schemas.",
  mkcert: "Make trusted HTTPS certificates for local development.",
  just: "A handy way to save and run project commands.",
  pandoc: "Convert documents between Markdown, Word, PDF, HTML, and more.",
  rust: "The Rust programming language.",
  terraform: "Define cloud infrastructure as code.",
  java: "The Java runtime and development kit.",
  ruby: "The Ruby programming language.",
  php: "The PHP programming language.",
  bun: "A fast all-in-one JavaScript runtime, bundler, and package manager.",
  deno: "A secure JavaScript and TypeScript runtime.",
  lazygit: "A friendly terminal interface for Git.",
  starship: "A fast, customizable shell prompt.",
  bat: "Like cat, with syntax highlighting and Git changes.",
  eza: "A modern ls with colors, icons, and Git status.",
  zoxide: "A smarter cd that remembers where you go.",
  fd: "A simple, fast alternative to find.",
  k9s: "A terminal dashboard for Kubernetes clusters.",
  postgres: "The PostgreSQL database server and tools.",
  redis: "In-memory data store for caching and queues.",
  mysql: "The MySQL database server and tools.",
  sqlite: "A small, fast, file-based SQL database.",
  hugo: "A fast static website generator.",
  "llama.cpp": "Run AI language models locally with llama.cpp.",
  claude: "Anthropic's Claude Code agent in your terminal.",
  codex: "OpenAI's Codex agent in your terminal.",
  opencode: "An open-source AI coding agent for the terminal.",
};

/** Everyday Unix tools mise doesn't version, installed as system packages
 *  (`mise bootstrap packages use -g …`): Homebrew on macOS, apt on Linux. */
export interface Utility {
  id: string;
  title: string;
  description: string;
  brew: string;
  apt: string | null;
  keywords: string[];
}

export const UTILITIES: Utility[] = [
  { id: "git", title: "git", description: "Version control. macOS includes it; this installs the latest.", brew: "git", apt: "git", keywords: ["version control"] },
  { id: "wget", title: "wget", description: "Download files from the web.", brew: "wget", apt: "wget", keywords: ["download"] },
  { id: "curl", title: "curl", description: "Transfer data with URLs. macOS includes it; this installs the latest.", brew: "curl", apt: "curl", keywords: ["http", "download"] },
  { id: "tree", title: "tree", description: "Show folders as a tree.", brew: "tree", apt: "tree", keywords: ["folders", "list"] },
  { id: "htop", title: "htop", description: "See what's using your CPU and memory.", brew: "htop", apt: "htop", keywords: ["processes", "monitor"] },
  { id: "gnupg", title: "GnuPG", description: "Encrypt files and sign Git commits (gpg).", brew: "gnupg", apt: "gnupg", keywords: ["gpg", "encrypt", "sign"] },
  { id: "rsync", title: "rsync", description: "Copy and sync folders efficiently.", brew: "rsync", apt: "rsync", keywords: ["copy", "sync", "backup"] },
  { id: "nmap", title: "nmap", description: "Scan networks and see open ports.", brew: "nmap", apt: "nmap", keywords: ["network", "ports"] },
  { id: "watch", title: "watch", description: "Run a command over and over and watch the output.", brew: "watch", apt: "procps", keywords: ["repeat"] },
  { id: "socat", title: "socat", description: "Relay data between network connections.", brew: "socat", apt: "socat", keywords: ["network"] },
  { id: "telnet", title: "telnet", description: "Open simple network connections to test services.", brew: "telnet", apt: "telnet", keywords: ["network"] },
  { id: "gnu-sed", title: "GNU sed", description: "The GNU version of sed, as Linux has it (gsed).", brew: "gnu-sed", apt: null, keywords: ["sed", "text"] },
  { id: "gnu-tar", title: "GNU tar", description: "The GNU version of tar, as Linux has it (gtar).", brew: "gnu-tar", apt: null, keywords: ["tar", "archive"] },
  { id: "p7zip", title: "7-Zip", description: "Open and create .7z and other archives.", brew: "p7zip", apt: "p7zip-full", keywords: ["zip", "archive", "7z"] },
  { id: "aria2", title: "aria2", description: "Fast downloads over many connections.", brew: "aria2", apt: "aria2", keywords: ["download"] },
  { id: "parallel", title: "GNU parallel", description: "Run commands in parallel.", brew: "parallel", apt: "parallel", keywords: ["parallel"] },
  { id: "ncdu", title: "ncdu", description: "Find what's taking up disk space.", brew: "ncdu", apt: "ncdu", keywords: ["disk", "space"] },
  { id: "pv", title: "pv", description: "Show progress for data moving through a pipe.", brew: "pv", apt: "pv", keywords: ["progress"] },
  { id: "httpie", title: "HTTPie", description: "A friendly command for calling web APIs.", brew: "httpie", apt: "httpie", keywords: ["http", "api"] },
  { id: "tailscale", title: "Tailscale", description: "A private network between your devices.", brew: "tailscale", apt: null, keywords: ["vpn", "network"] },
  { id: "graphviz", title: "Graphviz", description: "Draw diagrams and graphs from text (dot).", brew: "graphviz", apt: "graphviz", keywords: ["diagram", "dot"] },
  { id: "ghostscript", title: "Ghostscript", description: "Work with PDF and PostScript files (gs).", brew: "ghostscript", apt: "ghostscript", keywords: ["pdf"] },
  { id: "entr", title: "entr", description: "Rerun a command when files change.", brew: "entr", apt: "entr", keywords: ["watch", "files"] },
  { id: "zsh", title: "zsh", description: "The Z shell (macOS's default) — the latest version.", brew: "zsh", apt: "zsh", keywords: ["shell"] },
  { id: "fish", title: "fish", description: "A friendly shell with autosuggestions out of the box.", brew: "fish", apt: "fish", keywords: ["shell"] },
  { id: "bash", title: "bash", description: "The latest Bash (macOS ships an old one).", brew: "bash", apt: "bash", keywords: ["shell"] },
];

/** Popular tools that mise installs by full name from another ecosystem. */
export interface EcosystemTool {
  name: string;
  spec: string;
  description: string;
}

export const ECOSYSTEM: EcosystemTool[] = [
  { name: "fastlane", spec: "gem:fastlane", description: "Automate iOS and Android builds and releases." },
  { name: "jupyterlab", spec: "pipx:jupyterlab", description: "Notebooks for Python and data science." },
  { name: "glances", spec: "pipx:glances", description: "A system monitor for the terminal." },
  { name: "ansible-lint", spec: "pipx:ansible-lint", description: "Check Ansible playbooks for problems." },
  { name: "openai-whisper", spec: "pipx:openai-whisper", description: "Transcribe speech to text with Whisper." },
  { name: "black", spec: "pipx:black", description: "The Python code formatter." },
  { name: "prettier", spec: "npm:prettier", description: "Format JavaScript, CSS, Markdown, and more." },
  { name: "typescript", spec: "npm:typescript", description: "The TypeScript compiler (tsc)." },
];
