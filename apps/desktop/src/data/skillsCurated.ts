// Well-known agent skills to start with. Each is a folder with a SKILL.md in a
// public GitHub repository, installed with the skills CLI
// (`skills add <source> --skill <name>`). Browse many more at skills.sh.

export interface CuratedSkill {
  name: string;
  /** `owner/repo`. */
  source: string;
  description: string;
}

export const CURATED_SKILLS: CuratedSkill[] = [
  { name: "find-skills", source: "vercel-labs/skills", description: "Lets your agent search skills.sh and install a skill when it needs one." },
  { name: "frontend-design", source: "anthropics/skills", description: "Build distinctive, production-grade interfaces instead of generic ones." },
  { name: "skill-creator", source: "anthropics/skills", description: "Create, test, and improve your own skills." },
  { name: "webapp-testing", source: "anthropics/skills", description: "Test local web apps with Playwright: screenshots, logs, and interactions." },
  { name: "mcp-builder", source: "anthropics/skills", description: "Build MCP servers that give agents access to an API or service." },
  { name: "pdf", source: "anthropics/skills", description: "Read, fill, merge, and create PDF files." },
  { name: "docx", source: "anthropics/skills", description: "Create and edit Word documents, with tracked changes and comments." },
  { name: "xlsx", source: "anthropics/skills", description: "Create and analyze spreadsheets, with formulas and formatting." },
  { name: "pptx", source: "anthropics/skills", description: "Create and edit PowerPoint presentations." },
  { name: "react-best-practices", source: "vercel-labs/agent-skills", description: "React and Next.js performance guidance from Vercel." },
  { name: "web-design-guidelines", source: "vercel-labs/agent-skills", description: "Reviews UI code against web interface and accessibility guidelines." },
  { name: "systematic-debugging", source: "obra/superpowers", description: "Find the root cause of a bug before changing code." },
  { name: "test-driven-development", source: "obra/superpowers", description: "Write the failing test first, then the code." },
  { name: "brainstorming", source: "obra/superpowers", description: "Turn a rough idea into a design through questions before building." },
];
