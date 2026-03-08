import { useState, useCallback, useEffect } from "react";

/** A bookmarked message entry */
export interface Bookmark {
  messageId: string;
  sessionId: string;
  role: string;
  /** Short preview of the message text */
  preview: string;
  /** Unix timestamp (ms) when the bookmark was created */
  createdAt: number;
}

const STORAGE_KEY = "opman-bookmarks";

/** Load all bookmarks from localStorage */
function loadBookmarks(): Map<string, Bookmark> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const arr: Bookmark[] = JSON.parse(raw);
      const map = new Map<string, Bookmark>();
      for (const b of arr) map.set(b.messageId, b);
      return map;
    }
  } catch {
    /* ignore */
  }
  return new Map();
}

/** Save bookmarks to localStorage */
function saveBookmarks(map: Map<string, Bookmark>) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(Array.from(map.values())));
  } catch {
    /* ignore */
  }
}

/**
 * Hook for managing message bookmarks.
 * Bookmarks are persisted in localStorage.
 */
export function useBookmarks() {
  const [bookmarks, setBookmarks] = useState<Map<string, Bookmark>>(() => loadBookmarks());

  // Persist whenever bookmarks change
  useEffect(() => {
    saveBookmarks(bookmarks);
  }, [bookmarks]);

  /** Check if a message is bookmarked */
  const isBookmarked = useCallback(
    (messageId: string) => bookmarks.has(messageId),
    [bookmarks]
  );

  /** Toggle bookmark on a message */
  const toggleBookmark = useCallback(
    (messageId: string, sessionId: string, role: string, preview: string) => {
      setBookmarks((prev) => {
        const next = new Map(prev);
        if (next.has(messageId)) {
          next.delete(messageId);
        } else {
          next.set(messageId, {
            messageId,
            sessionId,
            role,
            preview: preview.slice(0, 120),
            createdAt: Date.now(),
          });
        }
        return next;
      });
    },
    []
  );

  /** Remove a bookmark */
  const removeBookmark = useCallback((messageId: string) => {
    setBookmarks((prev) => {
      const next = new Map(prev);
      next.delete(messageId);
      return next;
    });
  }, []);

  /** Get bookmarks for a specific session */
  const getSessionBookmarks = useCallback(
    (sessionId: string): Bookmark[] => {
      return Array.from(bookmarks.values())
        .filter((b) => b.sessionId === sessionId)
        .sort((a, b) => b.createdAt - a.createdAt);
    },
    [bookmarks]
  );

  /** All bookmarks, sorted by most recent */
  const allBookmarks = Array.from(bookmarks.values()).sort(
    (a, b) => b.createdAt - a.createdAt
  );

  return {
    bookmarks,
    allBookmarks,
    isBookmarked,
    toggleBookmark,
    removeBookmark,
    getSessionBookmarks,
  };
}
