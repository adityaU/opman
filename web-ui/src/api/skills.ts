import { apiDelete, apiFetch, apiPost, apiPut, apiUpload } from "./client";

/**
 * Skills: `~/.config/opman/skills/<name>/SKILL.md`, reachable by every runner through
 * the `opman mcp-skills` server.
 *
 * The name is the directory name and the identity — the backend parses it into a
 * validated type, so a traversal attempt is rejected at the extractor rather than by each
 * handler. `title` is display only and defaults to the name.
 */

export interface SkillSummary {
  name: string;
  title: string;
  description: string;
  /** `mcp.json` servers this skill needs, so a missing login can be named. */
  requires: string[];
}

export interface Skill extends SkillSummary {
  /** Everything below the frontmatter. */
  content: string;
}

export interface SkillDraft {
  name: string;
  title?: string;
  description: string;
  content: string;
  requires?: string[];
}

const skill = (name: string) => `/skills/${encodeURIComponent(name)}`;

/** `requires` is defaulted so the list can map over it without checking first. */
export async function fetchSkills(): Promise<SkillSummary[]> {
  const raw = await apiFetch<Partial<SkillSummary>[]>("/skills");
  return raw.map((entry) => ({
    name: entry.name ?? "",
    title: entry.title ?? "",
    description: entry.description ?? "",
    requires: entry.requires ?? [],
  }));
}

/** One skill in full. Resolves to null when the name is not in the registry. */
export async function fetchSkill(name: string): Promise<Skill | null> {
  const raw = await apiFetch<Partial<Skill> | null>(skill(name));
  if (!raw?.name) return null;
  return {
    name: raw.name,
    title: raw.title ?? "",
    description: raw.description ?? "",
    content: raw.content ?? "",
    requires: raw.requires ?? [],
  };
}

export async function createSkill(draft: SkillDraft): Promise<void> {
  await apiPost("/skills", draft);
}

export async function updateSkill(name: string, draft: SkillDraft): Promise<void> {
  await apiPut(skill(name), draft);
}

export async function deleteSkill(name: string): Promise<void> {
  await apiDelete(skill(name));
}

/**
 * Install skills from a zip.
 *
 * Each skill is a folder holding a `SKILL.md`; folders in the archive are unpacked into
 * the skills directory as-is.
 */
export async function uploadSkillsArchive(file: File): Promise<void> {
  const body = new FormData();
  body.append("skills_zip", file);
  await apiUpload("/skills/upload", body);
}
