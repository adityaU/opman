import { useCallback, useEffect, useRef, useState } from "react";
import {
  createSkill,
  deleteSkill,
  fetchSkill,
  fetchSkills,
  updateSkill,
  uploadSkillsArchive,
  type Skill,
  type SkillDraft,
  type SkillSummary,
} from "../../api/skills";

/**
 * The installed skills.
 *
 * Bodies are fetched one at a time: the list only needs frontmatter, and a skill's content
 * is an entire document. Nothing here caches a body — the editor asks for the one it is
 * about to show, so it can never open stale text over a file the agent has since rewritten.
 */

export interface SkillsState {
  readonly skills: readonly SkillSummary[];
  readonly loading: boolean;
  readonly error: string | undefined;
  readonly saving: boolean;
  readonly refresh: () => void;
  readonly load: (name: string) => Promise<Skill | null>;
  readonly save: (draft: SkillDraft, original?: string) => Promise<boolean>;
  readonly remove: (name: string) => Promise<void>;
  readonly upload: (file: File) => Promise<boolean>;
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useSkills(onError: (message: string) => void): SkillsState {
  const [skills, setSkills] = useState<readonly SkillSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();
  const [saving, setSaving] = useState(false);
  const alive = useRef(true);

  const refresh = useCallback(() => {
    fetchSkills()
      .then((list) => {
        if (!alive.current) return;
        setSkills(list);
        setError(undefined);
      })
      .catch((cause) => alive.current && setError(message(cause)))
      .finally(() => alive.current && setLoading(false));
  }, []);

  useEffect(() => {
    alive.current = true;
    refresh();
    return () => {
      alive.current = false;
    };
  }, [refresh]);

  const load = useCallback(
    async (name: string) => {
      try {
        return await fetchSkill(name);
      } catch (cause) {
        onError(message(cause));
        return null;
      }
    },
    [onError],
  );

  /** Run a write and refetch, reporting failure rather than leaving the list wrong. */
  const write = useCallback(
    async (action: () => Promise<void>): Promise<boolean> => {
      setSaving(true);
      try {
        await action();
        return true;
      } catch (cause) {
        onError(message(cause));
        return false;
      } finally {
        if (alive.current) {
          setSaving(false);
          refresh();
        }
      }
    },
    [onError, refresh],
  );

  /**
   * Create or update. `original` names the skill being edited — when the name changed, the
   * new one is written and the old directory removed, because the directory name *is* the
   * identity and leaving both behind would give the agent two copies of one skill.
   */
  const save = useCallback(
    (draft: SkillDraft, original?: string) =>
      write(async () => {
        if (!original) {
          await createSkill(draft);
          return;
        }
        if (original === draft.name) {
          await updateSkill(original, draft);
          return;
        }
        await createSkill(draft);
        await deleteSkill(original);
      }),
    [write],
  );

  const remove = useCallback(
    async (name: string) => {
      await write(() => deleteSkill(name));
    },
    [write],
  );

  const upload = useCallback((file: File) => write(() => uploadSkillsArchive(file)), [write]);

  return { skills, loading, error, saving, refresh, load, save, remove, upload };
}
