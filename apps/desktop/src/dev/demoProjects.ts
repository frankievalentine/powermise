// Demo projects for the browser stub: a trusted web app with tasks and env
// vars, and an untrusted infra repo whose hooks need reviewing before trust.
import type {
  ConfigPreview,
  EnvVarView,
  MiseAction,
  ProjectEntry,
  ProjectOverview,
  Task,
  TaskDetails,
  ToolInventory,
} from "../api";

const HOME = "/Users/demo";
const GLOBAL = `${HOME}/.config/mise/config.toml`;

interface DemoProject {
  path: string;
  trusted: boolean;
  tools: { name: string; requested: string; version: string; installed: boolean }[];
  env: EnvVarView[];
  tasks: (Task & { details: TaskDetails })[];
  preview: ConfigPreview;
  lastOpenedAt: number;
}

function task(
  project: string,
  name: string,
  description: string,
  run: string,
  extra: Partial<TaskDetails> = {},
): Task & { details: TaskDetails } {
  return {
    name,
    description,
    aliases: [],
    source: `${project}/mise.toml`,
    dir: project,
    hide: false,
    run: [run],
    details: { name, description, run: [run], depends: [], args: [], flags: [], ...extra },
  };
}

const WEB = `${HOME}/code/web-app`;
const INFRA = `${HOME}/code/infra`;

const projects: DemoProject[] = [
  {
    path: WEB,
    trusted: true,
    lastOpenedAt: Date.now() - 60_000,
    tools: [
      { name: "node", requested: "22", version: "22.20.0", installed: true },
      { name: "pnpm", requested: "10", version: "10.18.2", installed: true },
      { name: "python", requested: "3.12", version: "3.12.12", installed: false },
    ],
    env: [
      { name: "NODE_ENV", value: "development", source: `${WEB}/mise.toml`, secret: false },
      { name: "PORT", value: "5173", source: `${WEB}/mise.toml`, secret: false },
      { name: "DATABASE_URL", value: "postgres://localhost/web_dev", source: `${WEB}/mise.local.toml`, secret: false },
      { name: "STRIPE_SECRET_KEY", value: "sk_test_51Demo…", source: `${WEB}/mise.local.toml`, secret: true },
    ],
    tasks: [
      task(WEB, "dev", "Start the dev server with hot reload", "pnpm vite"),
      task(WEB, "test", "Run the test suite", "pnpm vitest run", {
        flags: [
          { name: "watch", usage: "--watch", help: "Re-run when files change", required: false, takesValue: false, choices: [], default: null },
          { name: "filter", usage: "--filter <pattern>", help: "Only tests whose name matches", required: false, takesValue: true, choices: [], default: null },
        ],
      }),
      task(WEB, "build", "Build for production", "pnpm vite build"),
      task(WEB, "deploy", "Deploy to an environment", "./scripts/deploy.sh {{arg(name='env')}}", {
        depends: ["build"],
        args: [
          { name: "env", usage: "<env>", help: "Where to deploy", required: true, takesValue: true, choices: ["staging", "production"], default: null },
        ],
        flags: [
          { name: "dry-run", usage: "--dry-run", help: "Show what would change without deploying", required: false, takesValue: false, choices: [], default: null },
        ],
      }),
    ],
    preview: { path: `${WEB}/mise.toml`, tools: [], envNames: [], envDirectives: [], hooks: [], tasks: [], parseError: null },
  },
  {
    path: INFRA,
    trusted: false,
    lastOpenedAt: Date.now() - 3_600_000,
    tools: [
      { name: "terraform", requested: "1.13", version: "1.13.4", installed: false },
      { name: "awscli", requested: "2", version: "2.31.0", installed: false },
    ],
    env: [
      { name: "AWS_PROFILE", value: "infra-admin", source: `${INFRA}/mise.toml`, secret: false },
      { name: "TF_VAR_region", value: "us-east-1", source: `${INFRA}/mise.toml`, secret: false },
    ],
    tasks: [
      task(INFRA, "plan", "Preview infrastructure changes", "terraform plan"),
      task(INFRA, "apply", "Apply infrastructure changes", "terraform apply"),
    ],
    preview: {
      path: `${INFRA}/mise.toml`,
      tools: [
        ["terraform", "1.13"],
        ["awscli", "2"],
      ],
      envNames: ["AWS_PROFILE", "TF_VAR_region"],
      envDirectives: ["_.file = .env", "_.source = ./scripts/aws-login.sh"],
      hooks: [
        ["enter", "./scripts/check-vpn.sh"],
        ["postinstall", "terraform init"],
      ],
      tasks: [
        ["plan", "terraform plan"],
        ["apply", "terraform apply"],
      ],
      parseError: null,
    },
  },
];

const find = (path: string) => projects.find((project) => project.path === path);
const nameOf = (path: string) => path.split("/").filter(Boolean).pop() ?? path;

function entry(project: DemoProject): ProjectEntry {
  return { path: project.path, name: nameOf(project.path), exists: true, lastOpenedAt: project.lastOpenedAt };
}

function overview(path: string): ProjectOverview {
  const project = find(path);
  if (!project) throw new Error(`${path} no longer exists`);
  project.lastOpenedAt = Date.now();
  const base = { path, name: nameOf(path), trust: { path, trusted: project.trusted }, problems: [] };
  if (!project.trusted) {
    return { ...base, previews: [project.preview], tools: null, configFiles: null, tasks: null, env: null };
  }
  const tools: ToolInventory = {};
  for (const tool of project.tools) {
    tools[tool.name] = [
      {
        version: tool.version,
        requested_version: tool.requested,
        install_path: `${HOME}/.local/share/mise/installs/${tool.name}/${tool.version}`,
        source: { type: "mise.toml", path: `${path}/mise.toml` },
        installed: tool.installed,
        active: tool.installed,
      },
    ];
  }
  tools.jq = [
    {
      version: "1.7.1",
      requested_version: "1.7.1",
      install_path: `${HOME}/.local/share/mise/installs/jq/1.7.1`,
      source: { type: "mise.toml", path: GLOBAL },
      installed: true,
      active: true,
    },
  ];
  return {
    ...base,
    previews: [],
    tools,
    configFiles: [
      { path: `${path}/mise.local.toml`, tools: [] },
      { path: `${path}/mise.toml`, tools: project.tools.map((tool) => tool.name) },
      { path: GLOBAL, tools: ["jq"] },
    ],
    tasks: project.tasks.map(({ details: _details, ...rest }) => rest),
    env: project.env,
  };
}

const fileFlag = (project: string, file: string | null) =>
  file ? ` --file ${file.startsWith(project + "/") ? file.slice(project.length + 1) : file}` : "";

export function describeProjectAction(action: MiseAction): string | undefined {
  const at = (path: string) => `cd ${path.replace(HOME, "~")} && `;
  switch (action.kind) {
    case "installProject":
      return `${at(action.project)}mise install`;
    case "trustProject":
      return `${at(action.project)}mise trust`;
    case "runTask":
      return `${at(action.project)}mise run ${action.task}${action.args.length ? ` -- ${action.args.join(" ")}` : ""}`;
    case "useInProject":
      return `${at(action.project)}mise use ${action.tool}@${action.version}`;
    case "setEnv":
      return `${at(action.project)}mise set${fileFlag(action.project, action.file)} ${action.key}=•••`;
    case "unsetEnv":
      return `${at(action.project)}mise unset${fileFlag(action.project, action.file)} ${action.key}`;
    default:
      return undefined;
  }
}

/** Applies a project action to the demo state; returns the output lines. */
export function performProjectAction(action: MiseAction): string[] | undefined {
  if (!("project" in action)) return undefined;
  const project = find(action.project);
  if (!project) return [`mise ERROR ${action.project} not found`];
  switch (action.kind) {
    case "installProject": {
      const missing = project.tools.filter((tool) => !tool.installed);
      missing.forEach((tool) => (tool.installed = true));
      return missing.length
        ? missing.flatMap((tool) => [
            `mise ${tool.name}@${tool.version}  download`,
            `mise ${tool.name}@${tool.version}  ✓ installed`,
          ])
        : ["mise all tools are installed"];
    }
    case "trustProject":
      project.trusted = true;
      return [`mise trusted ${action.project.replace(HOME, "~")}`];
    case "runTask": {
      const definition = project.tasks.find((entry) => entry.name === action.task);
      const command = definition?.run[0] ?? action.task;
      const lines = [`[${action.task}] $ ${command}${action.args.length ? " " + action.args.join(" ") : ""}`];
      if (action.task === "test") lines.push(" ✓ src/app.test.ts (12 tests)", " ✓ src/api.test.ts (8 tests)", "Test Files  2 passed", "Tests  20 passed");
      else if (action.task === "build") lines.push("vite v7.1.12 building for production…", "✓ 214 modules transformed.", "dist/index.html  0.46 kB", "✓ built in 1.84s");
      else if (action.task === "deploy") lines.push(`Deploying to ${action.args.find((arg) => !arg.startsWith("-")) ?? "staging"}${action.args.includes("--dry-run") ? " (dry run)" : ""}…`, "✓ 3 services updated");
      else lines.push("  VITE v7.1.12  ready in 312 ms", "  ➜  Local:   http://localhost:5173/");
      return lines;
    }
    case "useInProject":
      project.tools.push({ name: action.tool, requested: action.version, version: action.version, installed: true });
      return [`mise ${action.tool}@${action.version}  ✓ installed`, `mise ${project.path.replace(HOME, "~")}/mise.toml tools: ${action.tool}@${action.version}`];
    case "setEnv":
      project.env = [
        ...project.env.filter((entry) => entry.name !== action.key),
        { name: action.key, value: action.value, source: action.file ?? `${project.path}/mise.toml`, secret: /SECRET|TOKEN|PASSWORD|_KEY$/i.test(action.key) },
      ];
      return [`mise ${project.path.replace(HOME, "~")}/mise.toml env: ${action.key}`];
    case "unsetEnv":
      project.env = project.env.filter((entry) => entry.name !== action.key);
      return [`mise ${project.path.replace(HOME, "~")}/mise.toml removed env: ${action.key}`];
    default:
      return undefined;
  }
}

const NOT_MINE = Symbol("not mine");

export function demoProjects(command: string, args: Record<string, unknown>): unknown {
  switch (command) {
    case "list_projects":
      return [...projects].sort((a, b) => b.lastOpenedAt - a.lastOpenedAt).map(entry);
    case "add_project": {
      // No folder picker in a browser: add a third demo project.
      const path = (args.path as string | null) ?? `${HOME}/code/cli-tool`;
      if (!find(path)) {
        projects.push({
          path,
          trusted: true,
          lastOpenedAt: Date.now(),
          tools: [{ name: "rust", requested: "1.91", version: "1.91.0", installed: true }],
          env: [],
          tasks: [task(path, "test", "Run cargo tests", "cargo test")],
          preview: { path: `${path}/mise.toml`, tools: [], envNames: [], envDirectives: [], hooks: [], tasks: [], parseError: null },
        });
      }
      return entry(find(path)!);
    }
    case "forget_project": {
      const index = projects.findIndex((project) => project.path === args.path);
      if (index >= 0) projects.splice(index, 1);
      return undefined;
    }
    case "project_overview":
      return overview(args.path as string);
    case "task_details": {
      const details = find(args.path as string)?.tasks.find((entry) => entry.name === args.task)?.details;
      if (!details) throw new Error(`task ${args.task} not found`);
      return details;
    }
    default:
      return NOT_MINE;
  }
}

export const NOT_A_PROJECT_COMMAND = NOT_MINE;
