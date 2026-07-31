import {
  Children,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type DragEvent as ReactDragEvent,
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { getAppInfo, type AppInfoDto } from "./app-info";
import {
  layoutCommitGraph,
  type CommitGraphRow,
  type GraphSegment,
} from "./commitGraph";
import {
  closeAppWindow,
  minimizeAppWindow,
  toggleMaximizeAppWindow,
} from "./windowControls";
import {
  applyPatchSelection,
  addSubmodule,
  addRemote,
  abortMerge,
  abortRebase,
  activateSessionTab,
  activateWorktree,
  applyStash,
  cancelOperation,
  checkoutBranch,
  chooseCloneParentDirectory,
  chooseRepositoryDirectory,
  closeSessionTab,
  createBranch,
  createCommit,
  createStash,
  createTag,
  continueRebase,
  deleteBranch,
  deleteTag,
  deinitializeSubmodule,
  discardPath,
  dropStash,
  fastForwardBranch,
  getDiff,
  getCommitDiff,
  getCommitFiles,
  getHistoryPage,
  getGitIdentity,
  getDiagnostics,
  getOperationHistory,
  getRemotes,
  getRemoteTags,
  getReferences,
  getRepositorySidebar,
  getRepositorySnapshot,
  initializeSubmodule,
  listenForRepositoryChanges,
  normalizeAppError,
  openRepository,
  previewInteractiveRebase,
  rebaseBranch,
  renameBranch,
  reorderSessionTabs,
  removeSubmodule,
  removeRemote,
  resolveConflict,
  restoreSession,
  startClone,
  startInteractiveRebase,
  startRemoteOperation,
  skipRebase,
  stagePaths,
  unstagePaths,
  updateSessionTab,
  updateRemote,
  updateGlobalGitIdentity,
  updateRepositoryGitIdentity,
  type AppErrorDto,
  type CommitDto,
  type CommitFileDto,
  type DiffDto,
  type DiffTarget,
  type FileChangeDto,
  type GitRemoteDto,
  type GitIdentitySettingsDto,
  type RepositoryGitIdentityDto,
  type InteractiveRebaseAction,
  type InteractiveRebasePreviewDto,
  type OperationEventDto,
  type OperationRecordDto,
  type RemoteOperationOptions,
  type RemoteReferenceDeleteTarget,
  type RemoteTagDto,
  type RepositorySnapshotDto,
  type RepositorySidebarDto,
  type ReferenceDto,
  type SessionTabDto,
} from "./repository";
import { updateRepositoryOperation } from "./remote-operations";
import { localeTag, t } from "./i18n";
import {
  coAuthorsFromCommitBody,
  gravatarUrl,
  type GravatarAuthor,
} from "./gravatar";

type Page = "changes" | "history";
type AppInfoState =
  | { status: "loading" }
  | { status: "ready"; value: AppInfoDto }
  | { status: "error"; message: string };

const navigation: ReadonlyArray<{ id: Page; label: string; shortcut: string }> = [
  { id: "changes", label: t("Changes"), shortcut: "⌘1" },
  { id: "history", label: t("History"), shortcut: "⌘2" },
];

const alphaFeaturesEnabled = import.meta.env.DEV;
const openHistorySearchEvent = "gitacorn:open-history-search";
const openHistoryFileSearchEvent = "gitacorn:open-history-file-search";
const openChangesFileSearchEvent = "gitacorn:open-changes-file-search";
const openChangesDiffSearchEvent = "gitacorn:open-changes-diff-search";
type FindScope =
  | "sidebar"
  | "changes-files"
  | "changes-diff"
  | "history"
  | "history-files";

let repositoryConfirmationActive = false;

function confirmRepositoryMutation(message: string) {
  repositoryConfirmationActive = true;
  try {
    return window.confirm(message);
  } finally {
    window.setTimeout(() => {
      repositoryConfirmationActive = false;
    }, 0);
  }
}

type MultiSelection = ReturnType<typeof useMultiSelection>;

type ReferenceContextMenu =
  | { x: number; y: number; kind: "branch"; name: string; upstream?: string }
  | { x: number; y: number; kind: "tag"; name: string };

type StashItem = RepositorySidebarDto["stashes"][number];
type SubmoduleItem = NonNullable<RepositorySidebarDto["submodules"]>[number];

type StashContextMenu = {
  x: number;
  y: number;
  stash: StashItem;
};

type SubmoduleContextMenu = {
  x: number;
  y: number;
  submodule: SubmoduleItem;
};

type ReferenceEditor =
  | { mode: "createBranch"; source: string }
  | { mode: "renameBranch"; name: string; upstream?: string }
  | { mode: "createTag"; target: string };

type ReferenceDeleteDialogState = {
  kind: "branch" | "tag";
  repoId: string;
  name: string;
  remoteReferences: RemoteReferenceDeleteTarget[];
  loading: boolean;
};

type CheckoutTarget = {
  name: string;
  kind: "localBranch" | "remoteBranch" | "tag";
};

function useMultiSelection(items: string[], scope: string) {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [focused, setFocused] = useState<string | undefined>(undefined);
  const anchor = useRef<string | undefined>(undefined);
  const dragging = useRef(false);
  const dragBase = useRef<Set<string>>(new Set());
  const pointerStart = useRef<{ x: number; y: number } | undefined>(undefined);
  const pointerMoved = useRef(false);

  useEffect(() => {
    const valid = new Set(items);
    setSelected((current) => {
      const next = new Set([...current].filter((item) => valid.has(item)));
      return next.size === current.size ? current : next;
    });
    if (anchor.current && !valid.has(anchor.current)) anchor.current = undefined;
    setFocused((current) => (current && valid.has(current) ? current : items[0]));
  }, [items]);

  useEffect(() => {
    const continueDragging = (event: MouseEvent) => {
      if (!dragging.current || (event.buttons & 1) === 0) return;
      const start = pointerStart.current;
      if (
        start &&
        Math.abs(event.clientX - start.x) + Math.abs(event.clientY - start.y) > 4
      ) {
        pointerMoved.current = true;
      }
      const itemElement = document
        .elementFromPoint(event.clientX, event.clientY)
        ?.closest<HTMLElement>("[data-selection-scope][data-selection-index]");
      if (itemElement?.dataset.selectionScope !== scope) return;
      const itemIndex = Number(itemElement.dataset.selectionIndex);
      const item = items[itemIndex];
      if (!item) return;
      const anchorIndex = Math.max(0, items.indexOf(anchor.current ?? item));
      const range = items.slice(
        Math.min(anchorIndex, itemIndex),
        Math.max(anchorIndex, itemIndex) + 1,
      );
      setSelected(new Set([...dragBase.current, ...range]));
    };
    const stopDragging = () => {
      dragging.current = false;
    };
    window.addEventListener("mousemove", continueDragging);
    window.addEventListener("mouseup", stopDragging);
    return () => {
      window.removeEventListener("mousemove", continueDragging);
      window.removeEventListener("mouseup", stopDragging);
    };
  }, [items, scope]);

  const rangeTo = (item: string) => {
    const from = Math.max(0, items.indexOf(anchor.current ?? item));
    const to = Math.max(0, items.indexOf(item));
    return items.slice(Math.min(from, to), Math.max(from, to) + 1);
  };

  const onMouseDown = (item: string, event: ReactMouseEvent<HTMLElement>) => {
    if (event.button !== 0) return;
    event.currentTarget.focus();
    setFocused(item);
    pointerStart.current = { x: event.clientX, y: event.clientY };
    pointerMoved.current = false;
    const additive = event.ctrlKey || event.metaKey;
    if (event.shiftKey) {
      const range = rangeTo(item);
      setSelected((current) =>
        new Set(additive ? [...current, ...range] : range),
      );
    } else if (additive) {
      setSelected((current) => {
        const next = new Set(current);
        if (next.has(item)) next.delete(item);
        else next.add(item);
        return next;
      });
      anchor.current = item;
    } else {
      if (!selected.has(item)) {
        setSelected(new Set([item]));
      }
      anchor.current = item;
    }
    dragBase.current = new Set(additive ? selected : []);
    dragging.current = true;
  };

  const onClick = (item: string, event: ReactMouseEvent<HTMLElement>) => {
    const additive = event.ctrlKey || event.metaKey;
    if (!pointerMoved.current && !event.shiftKey && !additive) {
      setSelected(new Set([item]));
      anchor.current = item;
    }
    setFocused(item);
    pointerStart.current = undefined;
  };

  const onMouseEnter = (item: string, event: ReactMouseEvent<HTMLElement>) => {
    if (!dragging.current || (event.buttons & 1) === 0) return;
    const range = rangeTo(item);
    setSelected(new Set([...dragBase.current, ...range]));
  };

  const onKeyDown = (
    item: string,
    event: ReactKeyboardEvent<HTMLElement>,
    onActivate?: (item: string) => void,
    onFocusIndex?: (index: number) => void,
  ) => {
    const additive = event.ctrlKey || event.metaKey;
    if (additive && event.key.toLowerCase() === "a") {
      event.preventDefault();
      setSelected(new Set(items));
      return;
    }
    if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault();
      const current = Math.max(0, items.indexOf(item));
      const nextIndex = Math.max(
        0,
        Math.min(items.length - 1, current + (event.key === "ArrowDown" ? 1 : -1)),
      );
      const nextItem = items[nextIndex];
      if (!nextItem) return;
      setFocused(nextItem);
      if (event.shiftKey) {
        const range = rangeTo(nextItem);
        setSelected((currentSelection) =>
          new Set(additive ? [...currentSelection, ...range] : range),
        );
      } else if (!additive) {
        setSelected(new Set([nextItem]));
        anchor.current = nextItem;
      }
      onFocusIndex?.(nextIndex);
      if (!additive || event.shiftKey) onActivate?.(nextItem);
      return;
    }
    if (event.key === " ") {
      event.preventDefault();
      if (additive) {
        setSelected((current) => {
          const next = new Set(current);
          if (next.has(item)) next.delete(item);
          else next.add(item);
          return next;
        });
      } else {
        setSelected(new Set([item]));
      }
      anchor.current = item;
      setFocused(item);
      onActivate?.(item);
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      if (!selected.has(item)) setSelected(new Set([item]));
      anchor.current = item;
      setFocused(item);
      onActivate?.(item);
    }
  };

  const clear = () => {
    setSelected(new Set());
    anchor.current = undefined;
  };

  return {
    items,
    selected,
    focused,
    setFocused,
    setSelected,
    clear,
    onMouseDown,
    onMouseEnter,
    onClick,
    onKeyDown,
  };
}

export type ThemeSetting = "system" | "light" | "dark";

export function App() {
  const [appInfo, setAppInfo] = useState<AppInfoState>({ status: "loading" });
  const [tabs, setTabs] = useState<SessionTabDto[]>([]);
  const repositoryTabsRef = useRef<HTMLDivElement>(null);
  const [tabScrollState, setTabScrollState] = useState({
    canScrollLeft: false,
    canScrollRight: false,
  });
  const [draggedTabId, setDraggedTabId] = useState<string>();
  const draggedTabIdRef = useRef<string | undefined>(undefined);
  const suppressTabClickRef = useRef<string | undefined>(undefined);
  const [tabDropTarget, setTabDropTarget] = useState<{
    repoId: string;
    edge: "before" | "after";
  }>();
  const [sessionLoading, setSessionLoading] = useState(true);
  const [opening, setOpening] = useState(false);
  const [refreshing, setRefreshing] = useState<Set<string>>(new Set());
  const [sidebars, setSidebars] = useState<Record<string, RepositorySidebarDto>>({});
  const [referencesMap, setReferencesMap] = useState<Record<string, ReferenceDto[]>>({});
  const [remoteTagsMap, setRemoteTagsMap] = useState<Record<string, RemoteTagDto[]>>({});
  const [loadingRemoteTags, setLoadingRemoteTags] = useState(false);
  const [remotes, setRemotes] = useState<GitRemoteDto[]>([]);
  const [remoteEditor, setRemoteEditor] = useState<{
    mode: "add" | "edit";
    remote?: GitRemoteDto;
  }>();
  const [showSubmoduleAdd, setShowSubmoduleAdd] = useState(false);
  const [remoteDialog, setRemoteDialog] = useState<"fetch" | "pull" | "push">();
  const [remoteContextMenu, setRemoteContextMenu] = useState<{
    x: number;
    y: number;
    remote?: GitRemoteDto;
  }>();
  const [referenceContextMenu, setReferenceContextMenu] =
    useState<ReferenceContextMenu>();
  const [stashContextMenu, setStashContextMenu] =
    useState<StashContextMenu>();
  const [submoduleContextMenu, setSubmoduleContextMenu] =
    useState<SubmoduleContextMenu>();
  const [stashDialog, setStashDialog] = useState<StashItem>();
  const [referenceEditor, setReferenceEditor] = useState<ReferenceEditor>();
  const [referenceDeleteDialog, setReferenceDeleteDialog] =
    useState<ReferenceDeleteDialogState>();
  const [checkoutTarget, setCheckoutTarget] = useState<CheckoutTarget>();
  const [error, setError] = useState<AppErrorDto>();
  const reportError = useCallback(
    (reason: unknown) => setError(normalizeAppError(reason)),
    [],
  );
  const [remoteOperations, setRemoteOperations] = useState<
    Record<string, OperationEventDto>
  >({});
  const [cloneUrl, setCloneUrl] = useState("");
  const [showClone, setShowClone] = useState(false);
  const [cloneOperation, setCloneOperation] = useState<OperationEventDto>();
  const [showSettings, setShowSettings] = useState(false);
  const [showRepositorySettings, setShowRepositorySettings] = useState(false);
  const [globalIdentityName, setGlobalIdentityName] = useState("");
  const [globalIdentityEmail, setGlobalIdentityEmail] = useState("");
  const [repositoryIdentity, setRepositoryIdentity] =
    useState<RepositoryGitIdentityDto>();
  const [repositoryIdentityName, setRepositoryIdentityName] = useState("");
  const [repositoryIdentityEmail, setRepositoryIdentityEmail] = useState("");
  const [overrideRepositoryName, setOverrideRepositoryName] = useState(false);
  const [overrideRepositoryEmail, setOverrideRepositoryEmail] = useState(false);
  const [identityLoading, setIdentityLoading] = useState(false);
  const [identitySaving, setIdentitySaving] = useState<"global" | "repository">();
  const [identityMessage, setIdentityMessage] = useState("");
  const [identityError, setIdentityError] = useState("");
  const [showOperationCenter, setShowOperationCenter] = useState(false);
  const [showSidebarSearch, setShowSidebarSearch] = useState(false);
  const [sidebarFilter, setSidebarFilter] = useState("");
  const sidebarSearchInputRef = useRef<HTMLInputElement>(null);
  const lastFindScopeRef = useRef<FindScope | undefined>(undefined);
  const [themeSetting, setThemeSetting] = useState<ThemeSetting>(() => {
    if (typeof window !== "undefined") {
      try {
        const saved = localStorage.getItem("gitacorn_theme");
        if (saved === "light" || saved === "dark" || saved === "system") {
          return saved;
        }
      } catch {
        // ignore
      }
    }
    return "system";
  });
  const [showGravatars, setShowGravatars] = useState(() => {
    if (typeof window !== "undefined") {
      try {
        return localStorage.getItem("gitacorn_show_gravatars") === "true";
      } catch {
        // ignore
      }
    }
    return false;
  });

  useEffect(() => {
    try {
      localStorage.setItem("gitacorn_show_gravatars", String(showGravatars));
    } catch {
      // ignore
    }
  }, [showGravatars]);

  useEffect(() => {
    try {
      localStorage.setItem("gitacorn_theme", themeSetting);
    } catch {
      // ignore
    }

    const applyTheme = () => {
      let activeTheme: "light" | "dark" = "dark";
      if (themeSetting === "system") {
        activeTheme = typeof window !== "undefined" && window.matchMedia?.("(prefers-color-scheme: dark)").matches
          ? "dark"
          : "light";
      } else {
        activeTheme = themeSetting;
      }
      document.documentElement.setAttribute("data-theme", activeTheme);
    };

    applyTheme();

    if (themeSetting === "system" && typeof window !== "undefined" && window.matchMedia) {
      const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      const handler = () => applyTheme();
      if (mediaQuery.addEventListener) {
        mediaQuery.addEventListener("change", handler);
        return () => mediaQuery.removeEventListener("change", handler);
      } else if (mediaQuery.addListener) {
        mediaQuery.addListener(handler);
        return () => mediaQuery.removeListener(handler);
      }
    }
  }, [themeSetting]);

  useEffect(() => {
    if (showSidebarSearch) {
      sidebarSearchInputRef.current?.focus();
    }
  }, [showSidebarSearch]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (showSubmoduleAdd) setShowSubmoduleAdd(false);
        else if (remoteEditor) setRemoteEditor(undefined);
        else if (remoteDialog) setRemoteDialog(undefined);
        else if (checkoutTarget) setCheckoutTarget(undefined);
        else if (stashDialog) setStashDialog(undefined);
        else if (referenceDeleteDialog) setReferenceDeleteDialog(undefined);
        else if (referenceEditor) setReferenceEditor(undefined);
        else if (stashContextMenu) setStashContextMenu(undefined);
        else if (submoduleContextMenu) setSubmoduleContextMenu(undefined);
        else if (referenceContextMenu) setReferenceContextMenu(undefined);
        else if (remoteContextMenu) setRemoteContextMenu(undefined);
        else if (showRepositorySettings) setShowRepositorySettings(false);
        else if (showSettings) setShowSettings(false);
        else if (showOperationCenter) setShowOperationCenter(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    referenceContextMenu,
    referenceDeleteDialog,
    referenceEditor,
    checkoutTarget,
    showSubmoduleAdd,
    stashContextMenu,
    submoduleContextMenu,
    stashDialog,
    remoteContextMenu,
    remoteDialog,
    remoteEditor,
    showRepositorySettings,
    showSettings,
    showOperationCenter,
  ]);

  const refreshRequests = useRef(new Map<string, boolean>());
  const activeTab = tabs.find((tab) => tab.active) ?? tabs[0];
  const activeSnapshot = activeTab?.snapshot;
  const activeRepoId = activeTab?.repoId;
  const activeRepoIdRef = useRef(activeRepoId);
  activeRepoIdRef.current = activeRepoId;
  const activeRepositoryReady = Boolean(activeSnapshot);
  const page: Page = activeTab?.page === "history" ? "history" : "changes";
  const activeSidebar = activeTab ? sidebars[activeTab.repoId] : undefined;

  useEffect(() => {
    const tabsElement = repositoryTabsRef.current;
    if (!tabsElement) return;
    const updateScrollState = () => {
      const maxScrollLeft = Math.max(
        0,
        tabsElement.scrollWidth - tabsElement.clientWidth,
      );
      setTabScrollState({
        canScrollLeft: tabsElement.scrollLeft > 1,
        canScrollRight: tabsElement.scrollLeft < maxScrollLeft - 1,
      });
    };
    updateScrollState();
    tabsElement.addEventListener("scroll", updateScrollState, {
      passive: true,
    });
    const observer =
      typeof ResizeObserver === "undefined"
        ? undefined
        : new ResizeObserver(updateScrollState);
    observer?.observe(tabsElement);
    const activeTabElement =
      tabsElement.querySelector<HTMLElement>(".repository-tab.active");
    if (activeTabElement) {
      const tabLeft = activeTabElement.offsetLeft;
      const tabRight = tabLeft + activeTabElement.offsetWidth;
      if (tabLeft < tabsElement.scrollLeft) {
        tabsElement.scrollTo({ left: tabLeft, behavior: "smooth" });
      } else if (tabRight > tabsElement.scrollLeft + tabsElement.clientWidth) {
        tabsElement.scrollTo({
          left: tabRight - tabsElement.clientWidth,
          behavior: "smooth",
        });
      }
    }
    return () => {
      tabsElement.removeEventListener("scroll", updateScrollState);
      observer?.disconnect();
    };
  }, [tabs]);

  function scrollRepositoryTabs(direction: -1 | 1) {
    const tabsElement = repositoryTabsRef.current;
    if (!tabsElement) return;
    tabsElement.scrollBy({
      left: direction * Math.max(180, tabsElement.clientWidth * 0.6),
      behavior: "smooth",
    });
  }

  const applyGitIdentity = useCallback((settings: GitIdentitySettingsDto) => {
    setGlobalIdentityName(settings.global.name ?? "");
    setGlobalIdentityEmail(settings.global.email ?? "");
    setRepositoryIdentity(settings.repository);
    setOverrideRepositoryName(Boolean(settings.repository?.local.name));
    setOverrideRepositoryEmail(Boolean(settings.repository?.local.email));
    setRepositoryIdentityName(
      settings.repository?.local.name ?? settings.repository?.effective.name ?? "",
    );
    setRepositoryIdentityEmail(
      settings.repository?.local.email ?? settings.repository?.effective.email ?? "",
    );
  }, []);

  const identityRepoId = showRepositorySettings ? activeRepoId : undefined;

  const reloadGitIdentity = useCallback(async () => {
    const settings = await getGitIdentity(identityRepoId);
    applyGitIdentity(settings);
  }, [applyGitIdentity, identityRepoId]);

  useEffect(() => {
    if (!showSettings && !showRepositorySettings) return;
    let active = true;
    setIdentityLoading(true);
    setIdentityError("");
    setIdentityMessage("");
    getGitIdentity(identityRepoId)
      .then((settings) => {
        if (active) applyGitIdentity(settings);
      })
      .catch((reason: unknown) => {
        if (active) setIdentityError(normalizeAppError(reason).message);
      })
      .finally(() => {
        if (active) setIdentityLoading(false);
      });
    return () => {
      active = false;
    };
  }, [
    applyGitIdentity,
    identityRepoId,
    showRepositorySettings,
    showSettings,
  ]);

  const handleSaveGlobalIdentity = async (event: FormEvent) => {
    event.preventDefault();
    setIdentitySaving("global");
    setIdentityError("");
    setIdentityMessage("");
    try {
      await updateGlobalGitIdentity({
        name: globalIdentityName.trim() || undefined,
        email: globalIdentityEmail.trim() || undefined,
      });
      await reloadGitIdentity();
      setIdentityMessage(t("Global Git identity saved."));
    } catch (reason) {
      setIdentityError(normalizeAppError(reason).message);
    } finally {
      setIdentitySaving(undefined);
    }
  };

  const handleSaveRepositoryIdentity = async (event: FormEvent) => {
    event.preventDefault();
    if (!activeRepoId) return;
    if (
      (overrideRepositoryName && !repositoryIdentityName.trim()) ||
      (overrideRepositoryEmail && !repositoryIdentityEmail.trim())
    ) {
      setIdentityError(t("Enabled repository overrides cannot be empty."));
      return;
    }
    setIdentitySaving("repository");
    setIdentityError("");
    setIdentityMessage("");
    try {
      await updateRepositoryGitIdentity(activeRepoId, {
        name: overrideRepositoryName
          ? repositoryIdentityName.trim()
          : undefined,
        email: overrideRepositoryEmail
          ? repositoryIdentityEmail.trim()
          : undefined,
      });
      await reloadGitIdentity();
      setIdentityMessage(t("Repository Git identity saved."));
    } catch (reason) {
      setIdentityError(normalizeAppError(reason).message);
    } finally {
      setIdentitySaving(undefined);
    }
  };

  useEffect(() => {
    const findScopeFor = (target: EventTarget | null): FindScope | undefined => {
      const element = target instanceof Element ? target : null;
      if (element?.closest(".sidebar")) {
        return "sidebar";
      }
      if (element?.closest(".file-panel")) {
        return "changes-files";
      }
      if (element?.closest(".diff-panel")) {
        return "changes-diff";
      }
      if (element?.closest(".commit-files")) {
        return "history-files";
      }
      if (element?.closest(".history-list-panel")) {
        return "history";
      }
      return undefined;
    };

    const rememberFindScope = (event: Event) => {
      const scope = findScopeFor(event.target);
      if (scope) {
        lastFindScopeRef.current = scope;
      }
    };

    const handleFindShortcut = (event: KeyboardEvent) => {
      if (
        !(event.ctrlKey || event.metaKey) ||
        event.key.toLocaleLowerCase() !== "f"
      ) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      const scope =
        lastFindScopeRef.current ??
        findScopeFor(document.activeElement) ??
        (page === "history" ? "history" : "changes-files");

      if (scope === "sidebar") {
        setShowSidebarSearch(true);
        window.setTimeout(() => sidebarSearchInputRef.current?.select(), 0);
        return;
      }

      if (scope === "changes-files") {
        window.dispatchEvent(new Event(openChangesFileSearchEvent));
        return;
      }

      if (scope === "changes-diff") {
        window.dispatchEvent(new Event(openChangesDiffSearchEvent));
        return;
      }

      if (scope === "history") {
        window.dispatchEvent(new Event(openHistorySearchEvent));
        return;
      }

      if (scope === "history-files") {
        window.dispatchEvent(new Event(openHistoryFileSearchEvent));
        return;
      }

      window.dispatchEvent(new Event(openChangesFileSearchEvent));
    };

    window.addEventListener("pointerdown", rememberFindScope, true);
    window.addEventListener("focusin", rememberFindScope, true);
    window.addEventListener("keydown", handleFindShortcut, true);
    return () => {
      window.removeEventListener("pointerdown", rememberFindScope, true);
      window.removeEventListener("focusin", rememberFindScope, true);
      window.removeEventListener("keydown", handleFindShortcut, true);
    };
  }, [page]);

  const remoteBranchItems = useMemo(() => {
    if (activeSidebar?.remoteBranches?.items) {
      return activeSidebar.remoteBranches.items;
    }
    if (activeTab?.repoId && referencesMap[activeTab.repoId]) {
      return referencesMap[activeTab.repoId]
        .filter((r) => r.kind === "remoteBranch")
        .map((r) => r.shortName);
    }
    return [];
  }, [activeSidebar, activeTab?.repoId, referencesMap]);

  const remoteNames = useMemo(() => {
    const names = new Set(remotes.map((remote) => remote.name));
    for (const branch of remoteBranchItems) {
      const remote = branch.split("/", 1)[0];
      if (remote) names.add(remote);
    }
    for (const tag of remoteTagsMap[activeTab?.repoId ?? ""] ?? []) {
      names.add(tag.remote);
    }
    return [...names].sort((left, right) => left.localeCompare(right));
  }, [activeTab?.repoId, remoteBranchItems, remoteTagsMap, remotes]);

  const sidebarFilterQuery = sidebarFilter.trim().toLocaleLowerCase();
  const filteredLocalBranchItems = useMemo(
    () =>
      (activeSidebar?.branches.items ?? []).filter((branch) =>
        branch.toLocaleLowerCase().includes(sidebarFilterQuery),
      ),
    [activeSidebar?.branches.items, sidebarFilterQuery],
  );
  const filteredLocalTags = useMemo(
    () =>
      (activeSidebar?.tags.items ?? []).filter((tag) =>
        tag.toLocaleLowerCase().includes(sidebarFilterQuery),
      ),
    [activeSidebar?.tags.items, sidebarFilterQuery],
  );
  const filteredStashes = useMemo(
    () =>
      (activeSidebar?.stashes ?? []).filter((stash) =>
        `${stash.reference} ${stash.message}`
          .toLocaleLowerCase()
          .includes(sidebarFilterQuery),
      ),
    [activeSidebar?.stashes, sidebarFilterQuery],
  );
  const filteredWorktrees = useMemo(
    () =>
      (activeSidebar?.worktrees ?? []).filter((worktree) =>
        `${worktree.branch ?? ""} ${worktree.path}`
          .toLocaleLowerCase()
          .includes(sidebarFilterQuery),
      ),
    [activeSidebar?.worktrees, sidebarFilterQuery],
  );
  const filteredSubmodules = useMemo(
    () =>
      (activeSidebar?.submodules ?? []).filter((submodule) =>
        submodule.path.toLocaleLowerCase().includes(sidebarFilterQuery),
      ),
    [activeSidebar?.submodules, sidebarFilterQuery],
  );
  const remoteSidebarEntries = useMemo(
    () =>
      remoteNames
        .map((remoteName) => {
          const remoteMatches = remoteName
            .toLocaleLowerCase()
            .includes(sidebarFilterQuery);
          const branches = remoteBranchItems
            .filter((branch) => branch.startsWith(`${remoteName}/`))
            .map((branch) => branch.slice(remoteName.length + 1))
            .filter(
              (branch) =>
                remoteMatches ||
                branch.toLocaleLowerCase().includes(sidebarFilterQuery),
            );
          const tags = (
            remoteTagsMap[activeTab?.repoId ?? ""] ?? []
          ).filter(
            (tag) =>
              tag.remote === remoteName &&
              (remoteMatches ||
                tag.name.toLocaleLowerCase().includes(sidebarFilterQuery)),
          );
          return { name: remoteName, branches, tags, remoteMatches };
        })
        .filter(
          (entry) =>
            !sidebarFilterQuery ||
            entry.remoteMatches ||
            entry.branches.length > 0 ||
            entry.tags.length > 0,
        ),
    [
      activeTab?.repoId,
      remoteBranchItems,
      remoteNames,
      remoteTagsMap,
      sidebarFilterQuery,
    ],
  );
  const localBranchTree = useMemo(
    () => buildBranchTree(filteredLocalBranchItems, false),
    [filteredLocalBranchItems],
  );
  const branchSelectionItems = useMemo(
    () => [
      ...(activeSidebar?.branches.items ?? []).map((branch) => `local:${branch}`),
      ...remoteBranchItems.map((branch) => `remote:${branch}`),
    ],
    [activeSidebar?.branches.items, remoteBranchItems],
  );
  const branchSelection = useMultiSelection(branchSelectionItems, "branches");
  const tagSelectionItems = useMemo(
    () => [
      ...(activeSidebar?.tags.items ?? []).map((tag) => `local:${tag}`),
      ...(remoteTagsMap[activeTab?.repoId ?? ""] ?? []).map(
        (tag) => `remote:${tag.remote}/${tag.name}`,
      ),
    ],
    [activeSidebar?.tags.items, activeTab?.repoId, remoteTagsMap],
  );
  const tagSelection = useMultiSelection(tagSelectionItems, "tags");

  const handleBranchCheckout = (branchName: string, isRemote = false) => {
    setCheckoutTarget({
      name: branchName,
      kind: isRemote ? "remoteBranch" : "localBranch",
    });
  };

  const handleFetchRemoteTags = (remote?: string) => {
    if (!activeTab) return;
    const repoId = activeTab.repoId;
    setLoadingRemoteTags(true);
    getRemoteTags(repoId, remote)
      .then((tags) =>
        setRemoteTagsMap((prev) => ({
          ...prev,
          [repoId]: remote
            ? [
                ...(prev[repoId] ?? []).filter((tag) => tag.remote !== remote),
                ...tags,
              ]
            : tags,
        })),
      )
      .catch((reason: unknown) => setError(normalizeAppError(reason)))
      .finally(() => setLoadingRemoteTags(false));
  };

  const [sidebarWidth, setSidebarWidth] = useState(() => {
    try {
      const saved = localStorage.getItem("gitacorn:sidebar-width");
      if (saved) {
        const parsed = parseInt(saved, 10);
        if (!isNaN(parsed) && parsed >= 150 && parsed <= 600) {
          return parsed;
        }
      }
    } catch {
      // ignore
    }
    return 206;
  });

  const handleSidebarMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = sidebarWidth;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const deltaX = moveEvent.clientX - startX;
      const nextWidth = Math.max(150, Math.min(600, startWidth + deltaX));
      setSidebarWidth(nextWidth);
      try {
        localStorage.setItem("gitacorn:sidebar-width", String(nextWidth));
      } catch {
        // ignore
      }
    };

    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  const handleSidebarKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      setSidebarWidth((prev) => {
        const next = Math.max(150, prev - 10);
        try {
          localStorage.setItem("gitacorn:sidebar-width", String(next));
        } catch {}
        return next;
      });
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      setSidebarWidth((prev) => {
        const next = Math.min(600, prev + 10);
        try {
          localStorage.setItem("gitacorn:sidebar-width", String(next));
        } catch {}
        return next;
      });
    }
  };

  useEffect(() => {
    document.documentElement.lang = localeTag();
    let active = true;
    getAppInfo()
      .then((value) => active && setAppInfo({ status: "ready", value }))
      .catch((reason: unknown) => {
        if (active) {
          setAppInfo({
            status: "error",
            message: reason instanceof Error ? reason.message : String(reason),
          });
        }
      });
    restoreSession()
      .then((session) => active && setTabs(session.tabs))
      .catch((reason: unknown) => active && setError(normalizeAppError(reason)))
      .finally(() => active && setSessionLoading(false));
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!activeTab?.snapshot) return;
    const repoId = activeTab.repoId;
    if (!sidebars[repoId]) {
      getRepositorySidebar(repoId)
        .then((sidebar) =>
          setSidebars((current) => ({ ...current, [repoId]: sidebar })),
        )
        .catch((reason: unknown) => setError(normalizeAppError(reason)));
    }
    if (!referencesMap[repoId]) {
      getReferences(repoId)
        .then((refs) =>
          setReferencesMap((current) => ({ ...current, [repoId]: refs })),
        )
        .catch(() => {});
    }
  }, [activeTab, sidebars, referencesMap]);

  useEffect(() => {
    if (!activeRepositoryReady || !activeRepoId) {
      setRemotes([]);
      return;
    }
    let active = true;
    getRemotes(activeRepoId)
      .then((items) => {
        if (active) setRemotes(items);
      })
      .catch((reason: unknown) => {
        if (active) reportError(reason);
      });
    return () => {
      active = false;
    };
  }, [activeRepoId, activeRepositoryReady, reportError]);

  useEffect(() => {
    if (
      !remoteContextMenu &&
      !referenceContextMenu &&
      !stashContextMenu &&
      !submoduleContextMenu
    ) return;
    const close = () => {
      setRemoteContextMenu(undefined);
      setReferenceContextMenu(undefined);
      setStashContextMenu(undefined);
      setSubmoduleContextMenu(undefined);
    };
    window.addEventListener("click", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("blur", close);
    };
  }, [referenceContextMenu, remoteContextMenu, stashContextMenu, submoduleContextMenu]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const refreshRepository = async (repoId: string) => {
      if (refreshRequests.current.has(repoId)) {
        refreshRequests.current.set(repoId, true);
        return;
      }
      refreshRequests.current.set(repoId, false);
      setRefreshing((current) => {
        if (current.has(repoId)) return current;
        return new Set(current).add(repoId);
      });

      try {
        do {
          refreshRequests.current.set(repoId, false);
          const snapshot = await getRepositorySnapshot(repoId);
          if (disposed) return;
          setTabs((current) =>
            current.map((tab) =>
              tab.repoId === repoId &&
              (!tab.snapshot || snapshot.revision >= tab.snapshot.revision)
                ? { ...tab, snapshot, unavailable: false }
                : tab,
            ),
          );
        } while (refreshRequests.current.get(repoId));
      } catch (reason: unknown) {
        if (!disposed) reportError(reason);
      } finally {
        refreshRequests.current.delete(repoId);
        if (!disposed) {
          setRefreshing((current) => {
            const next = new Set(current);
            next.delete(repoId);
            return next;
          });
        }
      }
    };
    listenForRepositoryChanges((repoId) => {
      void refreshRepository(repoId);
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    const refreshActiveRepository = () => {
      if (repositoryConfirmationActive) return;
      const repoId = activeRepoIdRef.current;
      if (repoId) void refreshRepository(repoId);
    };
    window.addEventListener("focus", refreshActiveRepository);
    return () => {
      disposed = true;
      refreshRequests.current.clear();
      unlisten?.();
      window.removeEventListener("focus", refreshActiveRepository);
    };
  }, [reportError]);

  async function handleOpenRepository() {
    try {
      const path = await chooseRepositoryDirectory();
      if (!path) return;
      await handleOpenRepositoryPath(path);
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
    } finally {
      setSessionLoading(false);
    }
  }

  async function handleOpenRepositoryPath(
    path: string,
    openedFrom?: { repositoryName: string; worktreePath: string },
  ) {
    try {
      setOpening(true);
      setError(undefined);
      const session = openedFrom
        ? await openRepository(path, openedFrom)
        : await openRepository(path);
      setTabs(session.tabs);
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
    } finally {
      setOpening(false);
      setSessionLoading(false);
    }
  }

  function handleRemote(
    kind: "fetch" | "pull" | "push",
    options: RemoteOperationOptions,
  ) {
    if (!activeTab) return;
    const repoId = activeTab.repoId;
    setError(undefined);
    startRemoteOperation(
      repoId,
      kind,
      (event) => {
        if (event.repoId !== repoId) return;
        setRemoteOperations((current) =>
          updateRepositoryOperation(current, repoId, event),
        );
        if (event.snapshot) {
          setTabs((current) =>
            current.map((tab) =>
              tab.repoId === repoId ? { ...tab, snapshot: event.snapshot } : tab,
            ),
          );
        }
        if (event.error) setError(event.error);
      },
      options,
    ).catch((reason: unknown) => setError(normalizeAppError(reason)));
  }

  async function handleClone() {
    const remoteUrl = cloneUrl.trim();
    if (!remoteUrl) return;
    try {
      const parent = await chooseCloneParentDirectory();
      if (!parent) return;
      const separator = parent.includes("\\") && !parent.includes("/") ? "\\" : "/";
      const destination = `${parent}${parent.endsWith("\\") || parent.endsWith("/") ? "" : separator}${cloneRepositoryName(remoteUrl)}`;
      setError(undefined);
      await startClone(remoteUrl, destination, (event) => {
        setCloneOperation(event);
        if (event.error) setError(event.error);
        if (event.state === "succeeded" && event.destination) {
          openRepository(event.destination)
            .then((session) => {
              setTabs(session.tabs);
              setShowClone(false);
              setCloneUrl("");
            })
            .catch((reason: unknown) => setError(normalizeAppError(reason)));
        }
      });
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
    }
  }

  function activateTab(repoId: string) {
    setTabs((current) =>
      current.map((tab) => ({ ...tab, active: tab.repoId === repoId })),
    );
    activateSessionTab(repoId).catch((reason: unknown) => setError(normalizeAppError(reason)));
  }

  async function closeTab(repoId: string) {
    try {
      const session = await closeSessionTab(repoId);
      setTabs(session.tabs);
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
    }
  }

  function reorderTab(
    repoId: string,
    targetRepoId: string,
    edge: "before" | "after",
  ) {
    if (repoId === targetRepoId) return;
    const next = tabs.filter((tab) => tab.repoId !== repoId);
    const tab = tabs.find((item) => item.repoId === repoId);
    const targetIndex = next.findIndex((item) => item.repoId === targetRepoId);
    if (!tab || targetIndex < 0) return;
    next.splice(targetIndex + (edge === "after" ? 1 : 0), 0, tab);
    setTabs(next);
    reorderSessionTabs(next.map((tab) => tab.repoId)).catch((reason: unknown) =>
      setError(normalizeAppError(reason)),
    );
  }

  function tabDropEdge(
    clientX: number,
    element: HTMLElement,
  ): "before" | "after" {
    const bounds = element.getBoundingClientRect();
    return clientX < bounds.left + bounds.width / 2 ? "before" : "after";
  }

  function finishTabDrag() {
    draggedTabIdRef.current = undefined;
    setDraggedTabId(undefined);
    setTabDropTarget(undefined);
  }

  function beginTabDrag(
    repoId: string,
    event: ReactMouseEvent<HTMLButtonElement>,
  ) {
    if (event.button !== 0) return;
    const pointerId = { x: event.clientX, y: event.clientY };
    let dragging = false;
    let dropTarget:
      | { repoId: string; edge: "before" | "after" }
      | undefined;

    const handleMouseMove = (moveEvent: MouseEvent) => {
      if (
        !dragging &&
        Math.abs(moveEvent.clientX - pointerId.x) +
          Math.abs(moveEvent.clientY - pointerId.y) <
          5
      ) {
        return;
      }

      if (!dragging) {
        dragging = true;
        draggedTabIdRef.current = repoId;
        setDraggedTabId(repoId);
      }

      const target = document
        .elementFromPoint(moveEvent.clientX, moveEvent.clientY)
        ?.closest<HTMLElement>(".repository-tab");
      const targetRepoId = target?.dataset.repoId;
      if (!target || !targetRepoId || targetRepoId === repoId) {
        dropTarget = undefined;
        setTabDropTarget(undefined);
      } else {
        dropTarget = {
          repoId: targetRepoId,
          edge: tabDropEdge(moveEvent.clientX, target),
        };
        setTabDropTarget(dropTarget);
      }
      moveEvent.preventDefault();
    };

    const handleMouseUp = () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      if (dragging && dropTarget) {
        reorderTab(repoId, dropTarget.repoId, dropTarget.edge);
      }
      if (dragging) {
        suppressTabClickRef.current = repoId;
        window.setTimeout(() => {
          if (suppressTabClickRef.current === repoId) {
            suppressTabClickRef.current = undefined;
          }
        }, 0);
      }
      finishTabDrag();
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }

  function updateActiveTab(
    patch: Partial<
      Pick<
        SessionTabDto,
        | "page"
        | "selectedPath"
        | "selectedDiff"
        | "historyCursor"
        | "selectedCommit"
        | "historyFilter"
      >
    >,
  ) {
    if (!activeTab) return;
    const next = { ...activeTab, ...patch };
    setTabs((current) => current.map((tab) => (tab.repoId === next.repoId ? next : tab)));
    updateSessionTab(
      next.repoId,
      next.page,
      next.selectedPath,
      next.selectedDiff,
      next.panelWidth,
      next.historyCursor,
      next.selectedCommit,
      next.historyFilter,
    ).catch(
      (reason: unknown) => setError(normalizeAppError(reason)),
    );
  }

  const handleSelectReference = async (
    refName: string,
    kind?: "localBranch" | "remoteBranch" | "tag",
    directOid?: string,
  ) => {
    if (!activeTab) return;
    const repoId = activeTab.repoId;

    let targetOid = directOid;
    let targetRefName = refName;

    let refs = referencesMap[repoId];
    if (!refs || refs.length === 0) {
      try {
        refs = await getReferences(repoId);
        setReferencesMap((prev) => ({ ...prev, [repoId]: refs }));
      } catch {
        refs = [];
      }
    }

    if (refs && refs.length > 0) {
      const match =
        refs.find((r) => {
          if (kind && r.kind !== kind) return false;
          return (
            r.shortName === refName ||
            r.fullName === refName ||
            r.fullName === `refs/heads/${refName}` ||
            r.fullName === `refs/remotes/${refName}` ||
            r.fullName === `refs/tags/${refName}`
          );
        }) ??
        refs.find(
          (r) =>
            r.shortName === refName ||
            r.fullName === refName ||
            r.fullName === `refs/heads/${refName}` ||
            r.fullName === `refs/remotes/${refName}` ||
            r.fullName === `refs/tags/${refName}`,
        );

      if (match) {
        targetOid = targetOid ?? match.oid;
        targetRefName = match.fullName;
      }
    }

    if (!targetRefName.startsWith("refs/")) {
      if (kind === "localBranch") {
        targetRefName = `refs/heads/${refName}`;
      } else if (kind === "remoteBranch") {
        targetRefName = `refs/remotes/${refName}`;
      } else if (kind === "tag") {
        targetRefName = `refs/tags/${refName}`;
      }
    }

    if (
      !targetOid &&
      activeSnapshot?.head?.name === refName &&
      activeSnapshot?.head?.oid
    ) {
      targetOid = activeSnapshot.head.oid;
    }

    updateActiveTab({
      page: "history",
      ...(targetOid ? { selectedCommit: targetOid } : {}),
    });
  };

  async function handleRemoteMutation(
    action: () => Promise<RepositorySnapshotDto>,
  ) {
    if (!activeTab) return;
    const repoId = activeTab.repoId;
    try {
      setError(undefined);
      const snapshot = await action();
      setTabs((current) =>
        current.map((tab) =>
          tab.repoId === repoId ? { ...tab, snapshot } : tab,
        ),
      );
      setSidebars((current) => {
        const next = { ...current };
        delete next[repoId];
        return next;
      });
      setReferencesMap((current) => {
        const next = { ...current };
        delete next[repoId];
        return next;
      });
      setRemoteTagsMap((current) => {
        const next = { ...current };
        delete next[repoId];
        return next;
      });
      setRemotes(await getRemotes(repoId));
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
      throw reason;
    }
  }

  const handleActivateTagSelection = (selectionKey: string) => {
    if (selectionKey.startsWith("local:")) {
      void handleSelectReference(selectionKey.slice("local:".length), "tag");
      return;
    }
    const remoteKey = selectionKey.slice("remote:".length);
    const remoteTag = (remoteTagsMap[activeTab?.repoId ?? ""] ?? []).find(
      (tag) => `${tag.remote}/${tag.name}` === remoteKey,
    );
    if (remoteTag) {
      void handleSelectReference(remoteTag.name, "tag", remoteTag.oid);
    }
  };

  const handleActivateBranchSelection = (selectionKey: string) => {
    if (selectionKey.startsWith("local:")) {
      void handleSelectReference(
        selectionKey.slice("local:".length),
        "localBranch",
      );
    } else if (selectionKey.startsWith("remote:")) {
      void handleSelectReference(
        selectionKey.slice("remote:".length),
        "remoteBranch",
      );
    }
  };

  async function handleWorktreeActivate(worktreeId: string) {
    if (!activeTab || activeTab.worktreeId === worktreeId) return;
    try {
      setError(undefined);
      const session = await activateWorktree(activeTab.repoId, worktreeId);
      setTabs(session.tabs);
      setSidebars((current) => {
        const next = { ...current };
        delete next[activeTab.repoId];
        return next;
      });
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
    }
  }

  function acceptWorkspaceSnapshot(snapshot: RepositorySnapshotDto) {
    const repoId = snapshot.repository.id;
    setTabs((current) =>
      current.map((tab) =>
        tab.repoId === repoId ? { ...tab, snapshot } : tab,
      ),
    );
    setSidebars((current) => {
      const next = { ...current };
      delete next[repoId];
      return next;
    });
    setReferencesMap((current) => {
      const next = { ...current };
      delete next[repoId];
      return next;
    });
  }

  function handleSubmoduleInitialize(path: string) {
    if (
      !activeSnapshot ||
      !confirmRepositoryMutation(
        t("Initialize submodule {path}?", { path }),
      )
    ) {
      return;
    }
    void handleWorkspaceMutation(() =>
      initializeSubmodule(
        activeSnapshot.repository.id,
        activeSnapshot.revision,
        path,
      ),
    );
  }

  async function handleSubmoduleOpen(submodule: {
    path: string;
    absolutePath: string;
    initialized: boolean;
  }) {
    if (!activeSnapshot) return;
    const openedFrom = {
      repositoryName: activeSnapshot.repository.name,
      worktreePath: activeSnapshot.repository.worktreePath,
    };

    if (!submodule.initialized) {
      if (
        !confirmRepositoryMutation(
          t("Initialize submodule {path} and open it as a repository?", {
            path: submodule.path,
          }),
        )
      ) {
        return;
      }
      try {
        setOpening(true);
        setError(undefined);
        const initialized = await initializeSubmodule(
          activeSnapshot.repository.id,
          activeSnapshot.revision,
          submodule.path,
        );
        acceptWorkspaceSnapshot(initialized);
      } catch (reason: unknown) {
        setError(normalizeAppError(reason));
        setOpening(false);
        return;
      }
    }

    await handleOpenRepositoryPath(submodule.absolutePath, openedFrom);
  }

  async function handleSubmoduleRemove(submodule: {
    path: string;
    absolutePath: string;
  }) {
    if (
      !activeSnapshot ||
      !confirmRepositoryMutation(
        t("Remove submodule {path}? Its worktree will be removed and the deletion staged.", {
          path: submodule.path,
        }),
      )
    ) {
      return;
    }
    try {
      setError(undefined);
      let mutationSnapshot = activeSnapshot;
      const openTab = tabs.find(
        (tab) => tab.worktreePath === submodule.absolutePath,
      );
      if (openTab) {
        const session = await closeSessionTab(openTab.repoId);
        setTabs(session.tabs);
        mutationSnapshot = await getRepositorySnapshot(
          activeSnapshot.repository.id,
        );
      }
      acceptWorkspaceSnapshot(
        await removeSubmodule(
          mutationSnapshot.repository.id,
          mutationSnapshot.revision,
          submodule.path,
        ),
      );
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
    }
  }

  async function handleSubmoduleDeinitialize(submodule: {
    path: string;
    absolutePath: string;
  }) {
    if (
      !activeSnapshot ||
      !confirmRepositoryMutation(
        t("Deinitialize submodule {path}? Its worktree will be removed.", {
          path: submodule.path,
        }),
      )
    ) {
      return;
    }
    try {
      setError(undefined);
      let mutationSnapshot = activeSnapshot;
      const openTab = tabs.find(
        (tab) => tab.worktreePath === submodule.absolutePath,
      );
      if (openTab) {
        const session = await closeSessionTab(openTab.repoId);
        setTabs(session.tabs);
        mutationSnapshot = await getRepositorySnapshot(
          activeSnapshot.repository.id,
        );
      }
      acceptWorkspaceSnapshot(
        await deinitializeSubmodule(
          mutationSnapshot.repository.id,
          mutationSnapshot.revision,
          submodule.path,
        ),
      );
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
    }
  }

  async function handleWorkspaceMutation(
    action: () => Promise<RepositorySnapshotDto>,
  ) {
    if (!activeTab) return;
    try {
      setError(undefined);
      acceptWorkspaceSnapshot(await action());
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
    }
  }

  function remoteBranchDeleteTargets(
    name: string,
    upstream?: string,
  ): RemoteReferenceDeleteTarget[] {
    const candidates = new Set(
      [upstream, ...remoteBranchItems].filter(
        (reference): reference is string => Boolean(reference),
      ),
    );
    const targets = new Map<string, RemoteReferenceDeleteTarget>();
    const remotesByLength = [...remoteNames].sort(
      (left, right) => right.length - left.length,
    );
    for (const reference of candidates) {
      const remote = remotesByLength.find((candidate) =>
        reference.startsWith(`${candidate}/`),
      );
      if (!remote) continue;
      const remoteName = reference.slice(remote.length + 1);
      if (reference !== upstream && remoteName !== name) continue;
      targets.set(`${remote}\0${remoteName}`, {
        remote,
        name: remoteName,
      });
    }
    return [...targets.values()];
  }

  function openBranchDeleteDialog(
    repoId: string,
    name: string,
    upstream?: string,
  ) {
    setReferenceDeleteDialog({
      kind: "branch",
      repoId,
      name,
      remoteReferences: remoteBranchDeleteTargets(name, upstream),
      loading: false,
    });
  }

  async function openTagDeleteDialog(
    repoId: string,
    name: string,
  ) {
    setReferenceDeleteDialog({
      kind: "tag",
      repoId,
      name,
      remoteReferences: [],
      loading: true,
    });
    let remoteTags = remoteTagsMap[repoId] ?? [];
    try {
      remoteTags = await getRemoteTags(repoId);
      setRemoteTagsMap((current) => ({ ...current, [repoId]: remoteTags }));
    } catch {
      // Keep the last successfully loaded remote tag list and allow local deletion.
    }
    const remoteReferences = remoteTags
      .filter((tag) => tag.name === name)
      .map(({ remote, name: remoteName }) => ({
        remote,
        name: remoteName,
      }));
    setReferenceDeleteDialog((current) =>
      current?.kind === "tag" &&
      current.repoId === repoId &&
      current.name === name
        ? { ...current, remoteReferences, loading: false }
        : current,
    );
  }

  async function handleReferenceDelete(
    request: ReferenceDeleteDialogState,
    deleteRemote: boolean,
  ) {
    const latestSnapshot = tabs.find(
      (tab) => tab.repoId === request.repoId,
    )?.snapshot;
    if (!latestSnapshot) return;
    const remoteReferences = deleteRemote ? request.remoteReferences : [];
    await handleWorkspaceMutation(() =>
      request.kind === "branch"
        ? deleteBranch(
            request.repoId,
            latestSnapshot.revision,
            request.name,
            remoteReferences,
          )
        : deleteTag(
            request.repoId,
            latestSnapshot.revision,
            request.name,
            remoteReferences,
          ),
    );
  }

  async function handleStashApply(reference: string, dropAfterApply: boolean) {
    if (!activeSnapshot) return false;
    const repoId = activeSnapshot.repository.id;
    try {
      setError(undefined);
      const applied = await applyStash(
        repoId,
        activeSnapshot.revision,
        reference,
      );
      acceptWorkspaceSnapshot(applied);
      if (dropAfterApply) {
        const dropped = await dropStash(repoId, applied.revision, reference);
        acceptWorkspaceSnapshot(dropped);
      }
      return true;
    } catch (reason: unknown) {
      setError(normalizeAppError(reason));
      return false;
    }
  }

  const closeSidebarSearch = () => {
    setShowSidebarSearch(false);
    setSidebarFilter("");
  };

  const handleSidebarKeyDownCapture = (
    event: ReactKeyboardEvent<HTMLElement>,
  ) => {
    if (
      (event.ctrlKey || event.metaKey) &&
      event.key.toLocaleLowerCase() === "f"
    ) {
      event.preventDefault();
      event.stopPropagation();
      setShowSidebarSearch(true);
      window.setTimeout(() => sidebarSearchInputRef.current?.select(), 0);
      return;
    }
    if (event.key === "Escape" && showSidebarSearch) {
      event.preventDefault();
      event.stopPropagation();
      closeSidebarSearch();
    }
  };

  const branchLabel = activeSnapshot
    ? activeSnapshot.head.kind === "branch"
      ? activeSnapshot.head.name
      : activeSnapshot.head.kind === "detached"
        ? `${t("Detached")} ${activeSnapshot.head.oid?.slice(0, 8) ?? ""}`
        : t("Unborn branch")
    : undefined;

  return (
    <div className="app-shell">
      <header className="titlebar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <span className="acorn-mark" aria-hidden="true"><span /></span>
          <span>GitAcorn</span>
          {alphaFeaturesEnabled && (
            <button
              className="alpha-badge"
              type="button"
              aria-label={t("Show operation history")}
              aria-pressed={showOperationCenter}
              title={t("Show operation history")}
              onClick={() => setShowOperationCenter((visible) => !visible)}
            >
              ALPHA
            </button>
          )}
        </div>
        <div className="window-drag-region" data-tauri-drag-region />
        <button
          className="titlebar-settings-button"
          type="button"
          aria-label={t("Repository settings")}
          title={
            activeSnapshot
              ? t("Repository settings for {name}", {
                  name: activeSnapshot.repository.name,
                })
              : t("Open a repository to use repository settings")
          }
          disabled={!activeSnapshot}
          onClick={() => setShowRepositorySettings(true)}
        >
          <span aria-hidden="true">🗂️</span>
          <span>{t("Repository settings")}</span>
        </button>
        <button
          className="titlebar-settings-button"
          type="button"
          aria-label={t("Settings")}
          title={t("Settings")}
          onClick={() => setShowSettings(true)}
        >
          <span aria-hidden="true">⚙️</span>
          <span>{t("Settings")}</span>
        </button>
        <div className="window-controls">
          <button
            className="window-control"
            type="button"
            aria-label={t("Minimize window")}
            onClick={() => runWindowCommand(minimizeAppWindow)}
          >
            <span className="window-control-icon minimize" aria-hidden="true" />
          </button>
          <button
            className="window-control"
            type="button"
            aria-label={t("Maximize or restore window")}
            onClick={() => runWindowCommand(toggleMaximizeAppWindow)}
          >
            <span className="window-control-icon maximize" aria-hidden="true" />
          </button>
          <button
            className="window-control close"
            type="button"
            aria-label={t("Close window")}
            onClick={() => runWindowCommand(closeAppWindow)}
          >
            <span className="window-control-icon close" aria-hidden="true" />
          </button>
        </div>
      </header>

      <div className="tabbar" aria-label={t("Repository tabs")}>
        <div className="repository-tab-strip">
          <button
            className="tab-scroll-button"
            type="button"
            aria-label={t("Scroll repository tabs left")}
            disabled={!tabScrollState.canScrollLeft}
            onClick={() => scrollRepositoryTabs(-1)}
          >
            ‹
          </button>
          <div
            className="repository-tabs"
            ref={repositoryTabsRef}
            onWheel={(event) => {
              if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
              event.preventDefault();
              event.currentTarget.scrollLeft += event.deltaY;
            }}
          >
            {tabs.length === 0 && (
              <div className="tabbar-empty">
                {sessionLoading ? t("Restoring session…") : t("No repositories open")}
              </div>
            )}
            {tabs.map((tab) => (
              <div
              className={`repository-tab ${tab.active ? "active" : ""} ${tab.unavailable ? "unavailable" : ""} ${draggedTabId === tab.repoId ? "dragging" : ""} ${tabDropTarget?.repoId === tab.repoId && draggedTabId !== tab.repoId ? `drop-${tabDropTarget.edge}` : ""}`}
              data-repo-id={tab.repoId}
              key={tab.repoId}
              title={
                tab.openedFrom
                  ? t("Submodule of {name} ({path})", {
                      name: tab.openedFrom.repositoryName,
                      path: tab.openedFrom.worktreePath,
                    })
                  : t("Drag {name} to reorder", {
                      name: repositoryName(tab.worktreePath),
                    })
              }
            >
              <button
                className="tab-main"
                type="button"
                aria-current={tab.active ? "page" : undefined}
                onClick={() => {
                  if (suppressTabClickRef.current === tab.repoId) {
                    suppressTabClickRef.current = undefined;
                    return;
                  }
                  activateTab(tab.repoId);
                }}
                onMouseDown={(event) => beginTabDrag(tab.repoId, event)}
              >
                <span className="repository-dot" aria-hidden="true" />
                <span className="tab-label">
                  <strong>{tab.snapshot?.repository.name ?? repositoryName(tab.worktreePath)}</strong>
                  {tab.openedFrom && (
                    <small>
                      {t("Submodule of {name}", {
                        name: tab.openedFrom.repositoryName,
                      })}
                    </small>
                  )}
                </span>
                <span>{tab.unavailable ? "!" : (tab.snapshot?.changes.length ?? 0)}</span>
              </button>
              <div className="tab-controls">
                <button type="button" aria-label={t("Close {name}", { name: repositoryName(tab.worktreePath) })} onClick={() => closeTab(tab.repoId)}>×</button>
              </div>
              </div>
            ))}
          </div>
          <button
            className="tab-scroll-button"
            type="button"
            aria-label={t("Scroll repository tabs right")}
            disabled={!tabScrollState.canScrollRight}
            onClick={() => scrollRepositoryTabs(1)}
          >
            ›
          </button>
        </div>
        <button className="control-button control-button--primary open-button" type="button" disabled={opening} onClick={handleOpenRepository}>
          <span aria-hidden="true">＋</span>{" "}{opening ? t("Opening…") : t("Open a repository")}
        </button>
        <button className="control-button control-button--secondary open-button" type="button" onClick={() => setShowClone((value) => !value)}>
          {t("Clone")}
        </button>
      </div>

      <main
        className="workspace"
        style={{ "--sidebar-width": `${sidebarWidth}px` } as CSSProperties}
      >
        <aside
          className="sidebar"
          onKeyDownCapture={handleSidebarKeyDownCapture}
        >
          <nav aria-label={t("Repository navigation")}>
            <p className="section-label">{t("Workspace")}</p>
            {navigation.map((item) => (
              <button
                key={item.id}
                className={page === item.id ? "nav-item active" : "nav-item"}
                type="button"
                disabled={!activeTab}
                aria-current={page === item.id ? "page" : undefined}
                onClick={() => {
                  setShowOperationCenter(false);
                  updateActiveTab({ page: item.id });
                }}
              >
                <span className={`nav-icon ${item.id}`} aria-hidden="true" />
                <span>{item.label}</span><kbd>{item.shortcut}</kbd>
              </button>
            ))}
          </nav>
          {showSidebarSearch && (
            <div className="sidebar-searchbar" role="search">
              <input
                ref={sidebarSearchInputRef}
                className="control-input"
                type="search"
                aria-label={t("Filter sidebar")}
                placeholder={t("Filter branches, tags, stashes, submodules, or worktrees")}
                value={sidebarFilter}
                onChange={(event) =>
                  setSidebarFilter(event.currentTarget.value)
                }
              />
              <button
                className="search-close-button"
                type="button"
                aria-label={t("Close sidebar filter")}
                onClick={closeSidebarSearch}
              >
                ×
              </button>
            </div>
          )}
          <div className="sidebar-groups">
            <SidebarGroup
              label={t("Local Branches")}
              count={
                sidebarFilterQuery
                  ? filteredLocalBranchItems.length
                  : activeSidebar?.branches.total
              }
              initialLimit={999}
              onClearSelection={branchSelection.clear}
            >
              {localBranchTree.map((node) => (
                <BranchTreeNodeView
                  key={node.id}
                  node={node}
                  currentBranchLabel={branchLabel}
                  referencesList={activeTab ? referencesMap[activeTab.repoId] ?? [] : []}
                  isRemote={false}
                  selection={branchSelection}
                  selectionPrefix="local:"
                  onSelectSelectionKey={handleActivateBranchSelection}
                  onCheckout={(branchName) =>
                    handleBranchCheckout(branchName, false)
                  }
                  onSelect={(refName) => handleSelectReference(refName, "localBranch")}
                  onContextMenu={(event, refName) => {
                    event.preventDefault();
                    event.stopPropagation();
                    const reference = (activeTab
                      ? referencesMap[activeTab.repoId] ?? []
                      : []
                    ).find(
                      (item) =>
                        item.kind === "localBranch" &&
                        item.shortName === refName,
                    );
                    setReferenceContextMenu({
                      x: event.clientX,
                      y: event.clientY,
                      kind: "branch",
                      name: refName,
                      upstream: reference?.upstream,
                    });
                  }}
                />
              ))}
            </SidebarGroup>
            <SidebarGroup
              label={t("Remote")}
              count={remoteSidebarEntries.length}
              initialLimit={999}
              onClearSelection={() => {
                branchSelection.clear();
                tagSelection.clear();
              }}
              onContextMenu={(event) => {
                event.preventDefault();
                setRemoteContextMenu({
                  x: event.clientX,
                  y: event.clientY,
                });
              }}
            >
              {remoteSidebarEntries.map((remote) => (
                <RemoteReferenceNode
                  key={remote.name}
                  name={remote.name}
                  branches={remote.branches}
                  tags={remote.tags}
                  branchSelection={branchSelection}
                  tagSelection={tagSelection}
                  tagSelectionItems={tagSelectionItems}
                  onBranchSelection={handleActivateBranchSelection}
                  onTagSelection={handleActivateTagSelection}
                  onSelectBranch={(refName) =>
                    handleSelectReference(refName, "remoteBranch")
                  }
                  onSelectTag={(tag) =>
                    handleSelectReference(tag.name, "tag", tag.oid)
                  }
                  onCheckout={(branchName) =>
                    handleBranchCheckout(branchName, true)
                  }
                  onContextMenu={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    setRemoteContextMenu({
                      x: event.clientX,
                      y: event.clientY,
                      remote: remotes.find((item) => item.name === remote.name) ?? {
                        name: remote.name,
                        url: "",
                      },
                    });
                  }}
                />
              ))}
            </SidebarGroup>
            <SidebarGroup
              label={t("Tags")}
              count={
                sidebarFilterQuery
                  ? filteredLocalTags.length
                  : activeSidebar?.tags.total ?? 0
              }
              onClearSelection={tagSelection.clear}
            >
              {filteredLocalTags.map((tag) => (
                <div
                  key={tag}
                  className={`tag-item-row tree-leaf-row ${tagSelection.selected.has(`local:${tag}`) ? "selected" : ""}`}
                  role="button"
                  tabIndex={
                    tagSelection.focused === `local:${tag}` ||
                    (!tagSelection.focused &&
                      tagSelectionItems.indexOf(`local:${tag}`) === 0)
                      ? 0
                      : -1
                  }
                  aria-pressed={tagSelection.selected.has(`local:${tag}`)}
                  data-selection-scope="tags"
                  data-selection-index={tagSelectionItems.indexOf(`local:${tag}`)}
                  onMouseDown={(event) => tagSelection.onMouseDown(`local:${tag}`, event)}
                  onMouseEnter={(event) => tagSelection.onMouseEnter(`local:${tag}`, event)}
                  onClick={(e) => {
                    e.stopPropagation();
                    tagSelection.onClick(`local:${tag}`, e);
                    handleSelectReference(tag, "tag");
                  }}
                  onDoubleClick={(event) => {
                    event.stopPropagation();
                    setCheckoutTarget({ name: tag, kind: "tag" });
                  }}
                  onKeyDown={(event) => {
                    event.stopPropagation();
                    tagSelection.onKeyDown(
                      `local:${tag}`,
                      event,
                      handleActivateTagSelection,
                      (index) => focusSelectionIndex(event.currentTarget, index),
                    );
                  }}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    setReferenceContextMenu({
                      x: event.clientX,
                      y: event.clientY,
                      kind: "tag",
                      name: tag,
                    });
                  }}
                >
                  <span className="branch-icon" aria-hidden="true">🏷️ </span>
                  <span className="branch-name" title={tag}>{tag}</span>
                </div>
              ))}
            </SidebarGroup>
            <StashControls
              stashes={filteredStashes}
              onApply={setStashDialog}
              onContextMenu={(stash, x, y) =>
                setStashContextMenu({ stash, x, y })
              }
            />
            <SidebarGroup
              label={t("Submodules")}
              count={filteredSubmodules.length}
              initialLimit={999}
            >
              <button
                className="submodule-add-button"
                type="button"
                disabled={!activeSnapshot}
                onClick={() => setShowSubmoduleAdd(true)}
              >
                ＋ {t("Add submodule")}
              </button>
              {filteredSubmodules.map((submodule) => (
                <div
                  className="submodule-item"
                  key={submodule.path}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    setSubmoduleContextMenu({
                      submodule,
                      x: event.clientX,
                      y: event.clientY,
                    });
                  }}
                >
                  <button
                    className="submodule-main"
                    type="button"
                    title={
                      submodule.initialized
                        ? t("Double-click to open {path} as a repository", {
                            path: submodule.absolutePath,
                          })
                        : t("Double-click to initialize and open {path} as a repository", {
                            path: submodule.path,
                          })
                    }
                    disabled={opening}
                    onDoubleClick={() => {
                      void handleSubmoduleOpen(submodule);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        void handleSubmoduleOpen(submodule);
                      } else if (
                        event.key === "ContextMenu" ||
                        (event.shiftKey && event.key === "F10")
                      ) {
                        event.preventDefault();
                        const bounds = event.currentTarget.getBoundingClientRect();
                        setSubmoduleContextMenu({
                          submodule,
                          x: bounds.left + 12,
                          y: bounds.top + 12,
                        });
                      }
                    }}
                  >
                    <span className="submodule-icon" aria-hidden="true">◇</span>
                    <span>{submodule.path}</span>
                    {!submodule.initialized && (
                      <small>{t("not initialized")}</small>
                    )}
                  </button>
                  {!submodule.initialized && (
                    <button
                      className="submodule-action"
                      type="button"
                      aria-label={t("Initialize submodule {path}", {
                        path: submodule.path,
                      })}
                      title={t("Initialize")}
                      onClick={() => handleSubmoduleInitialize(submodule.path)}
                    >
                      ↻
                    </button>
                  )}
                  <button
                    className="submodule-action danger"
                    type="button"
                    aria-label={t("Remove submodule {path}", {
                      path: submodule.path,
                    })}
                    title={t("Remove")}
                    onClick={() => void handleSubmoduleRemove(submodule)}
                  >
                    ×
                  </button>
                </div>
              ))}
            </SidebarGroup>
            <SidebarGroup
              label={t("Worktrees")}
              count={filteredWorktrees.length}
            >
              {filteredWorktrees.map((worktree) => (
                <button
                  type="button"
                  key={worktree.id}
                  title={worktree.path}
                  aria-current={worktree.id === activeTab?.worktreeId ? "true" : undefined}
                  onClick={() => handleWorktreeActivate(worktree.id)}
                >
                  {worktree.isCurrent ? "● " : ""}{worktree.branch ?? t("Detached")}
                  {worktree.isLocked ? ` · ${t("locked")}` : ""}
                </button>
              ))}
            </SidebarGroup>
          </div>
          <div className="runtime-status" role="status">
            <span className={appInfo.status === "error" ? "status-dot error" : "status-dot"} />
            {appInfo.status === "loading" && t("Connecting to core…")}
            {appInfo.status === "ready" && `${appInfo.value.runtime} · v${appInfo.value.version}`}
            {appInfo.status === "error" && t("Core unavailable")}
          </div>
          <div
            className="sidebar-resizer"
            role="separator"
            aria-orientation="vertical"
            aria-label={t("Sidebar width")}
            tabIndex={0}
            onMouseDown={handleSidebarMouseDown}
            onKeyDown={handleSidebarKeyDown}
          />
        </aside>

        <section className="content" aria-live="polite">
          <div className="contextbar">
            <div>
              <span className="eyebrow">{activeTab?.worktreePath ?? t("Local workspace")}</span>
              <strong>
                {showOperationCenter
                  ? t("Operation center")
                  : activeSnapshot
                    ? `${branchLabel} · ${navigation.find((item) => item.id === page)?.label}`
                    : navigation.find((item) => item.id === page)?.label}
              </strong>
            </div>
            {!showOperationCenter && <div className="remote-actions" aria-label={t("Remote actions")}>
              {activeTab && refreshing.has(activeTab.repoId) && <span className="refreshing">{t("Refreshing…")}</span>}
              {activeTab && remoteOperations[activeTab.repoId] &&
                ["queued", "running"].includes(remoteOperations[activeTab.repoId].state) ? (
                  <>
                    <span className="refreshing" role="status">
                      {operationTerm(remoteOperations[activeTab.repoId].kind)} · {remoteOperations[activeTab.repoId].message ?? operationTerm(remoteOperations[activeTab.repoId].state)}
                    </span>
                    <button className="control-button control-button--secondary" type="button" onClick={() => cancelOperation(remoteOperations[activeTab.repoId].operationId).catch((reason: unknown) => setError(normalizeAppError(reason)))}>{t("Cancel")}</button>
                  </>
                ) : (
                  <>
                    <button className="control-button control-button--secondary" type="button" disabled={!activeTab} onClick={() => setRemoteDialog("fetch")}>{t("Fetch")}</button>
                    <button className="control-button control-button--secondary" type="button" disabled={!activeTab} onClick={() => setRemoteDialog("pull")}>{t("Pull")}{activeSnapshot?.behind ? ` ${activeSnapshot.behind}` : ""}</button>
                    <button className="control-button control-button--secondary" type="button" disabled={!activeTab} onClick={() => setRemoteDialog("push")}>{t("Push")}{activeSnapshot?.ahead ? ` ${activeSnapshot.ahead}` : ""}</button>
                  </>
                )}
            </div>}
          </div>
          {showClone && (
            <form className="clone-bar" onSubmit={(event) => { event.preventDefault(); void handleClone(); }}>
              <label htmlFor="clone-url">{t("Repository URL")}</label>
              <input className="control-input" id="clone-url" value={cloneUrl} onChange={(event) => setCloneUrl(event.target.value)} placeholder="https://host/owner/repository.git or git@host:owner/repository.git" />
              {cloneOperation && ["queued", "running"].includes(cloneOperation.state) ? (
                <>
                  <span role="status">{cloneOperation.message ?? t("Cloning…")}</span>
                  <button className="control-button control-button--secondary" type="button" onClick={() => cancelOperation(cloneOperation.operationId)}>{t("Cancel")}</button>
                </>
              ) : (
                <button className="control-button control-button--primary" type="submit" disabled={!cloneUrl.trim()}>{t("Choose destination and clone")}</button>
              )}
            </form>
          )}
          {appInfo.status === "error" && <ErrorBanner title={t("Could not reach the GitAcorn core.")} message={appInfo.message} />}
          {appInfo.status !== "error" && error && <ErrorBanner title={t("Repository session needs attention.")} message={error.message} detail={error.details} actionLabel={error.code === "repositoryNotFound" ? t("Choose another folder") : undefined} onAction={handleOpenRepository} />}
          {alphaFeaturesEnabled && showOperationCenter ? (
            <OperationsView onError={reportError} />
          ) : activeTab?.unavailable ? (
            <UnavailableRepository tab={activeTab} onLocate={handleOpenRepository} />
          ) : page === "changes" ? (
            activeSnapshot ? (
              <ChangesView
                snapshot={activeSnapshot}
                refreshing={refreshing.has(activeTab.repoId)}
                selectedPath={activeTab.selectedPath}
                panelWidth={activeTab.panelWidth}
                selectedTarget={activeTab.selectedDiff}
                onPanelWidth={(panelWidth) => {
                  const next = { ...activeTab, panelWidth };
                  setTabs((current) =>
                    current.map((tab) => (tab.repoId === next.repoId ? next : tab)),
                  );
                  updateSessionTab(
                    next.repoId,
                    next.page,
                    next.selectedPath,
                    next.selectedDiff,
                    panelWidth,
                    next.historyCursor,
                    next.selectedCommit,
                    next.historyFilter,
                  ).catch((reason: unknown) => setError(normalizeAppError(reason)));
                }}
                onSelect={(selectedPath, selectedDiff) =>
                  updateActiveTab({ selectedPath, selectedDiff })
                }
                onSnapshot={(snapshot) =>
                  {
                    setTabs((current) =>
                      current.map((tab) =>
                        tab.repoId === snapshot.repository.id &&
                        (!tab.snapshot || snapshot.revision >= tab.snapshot.revision)
                          ? { ...tab, snapshot, unavailable: false }
                          : tab,
                      ),
                    );
                    if (snapshot.stashCount !== activeSnapshot.stashCount) {
                      setSidebars((current) => {
                        const next = { ...current };
                        delete next[snapshot.repository.id];
                        return next;
                      });
                    }
                  }
                }
                onError={reportError}
              />
            ) : (
              <ChangesEmpty onOpen={handleOpenRepository} opening={opening || sessionLoading} />
            )
          ) : page === "history" ? (
            activeSnapshot && activeTab ? (
              <HistoryView
                key={activeTab.repoId}
                tab={activeTab}
                snapshot={activeSnapshot}
                showGravatars={showGravatars}
                onPersist={(patch) => updateActiveTab(patch)}
                onSnapshot={(snapshot) => {
                  setTabs((current) =>
                    current.map((tab) =>
                      tab.repoId === snapshot.repository.id &&
                      (!tab.snapshot || snapshot.revision >= tab.snapshot.revision)
                        ? { ...tab, snapshot, unavailable: false }
                        : tab,
                    ),
                  );
                }}
                onError={reportError}
                onClearError={() => setError(undefined)}
              />
            ) : (
              <HistoryEmpty />
            )
          ) : (
            <HistoryEmpty />
          )}
        </section>
      </main>
      {showSettings && (
        <div
          className="modal-overlay"
          onClick={() => setShowSettings(false)}
          role="presentation"
        >
          <div
            className="settings-modal git-settings-modal"
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-labelledby="settings-title"
          >
            <div className="settings-modal-header">
              <h2 id="settings-title">{t("Settings")}</h2>
              <button
                className="settings-close-btn"
                type="button"
                aria-label={t("Close settings")}
                onClick={() => setShowSettings(false)}
              >
                ×
              </button>
            </div>
            <div className="settings-modal-body">
              <div className="settings-section git-identity-settings">
                <h3>{t("Git author identity")}</h3>
                <p className="settings-section-desc">
                  {t("Configure the name and email Git uses for new commits.")}
                </p>
                {identityLoading ? (
                  <p className="identity-state">{t("Loading Git identity…")}</p>
                ) : (
                  <>
                    <form
                      className="identity-card"
                      aria-label={t("Global Git identity")}
                      onSubmit={handleSaveGlobalIdentity}
                    >
                      <div className="identity-card-heading">
                        <div>
                          <strong>{t("Global")}</strong>
                          <small>{t("Used when a repository has no override.")}</small>
                        </div>
                        <span className="identity-scope-badge">--global</span>
                      </div>
                      <label>
                        <span>{t("User name")}</span>
                        <input
                          value={globalIdentityName}
                          onChange={(event) =>
                            setGlobalIdentityName(event.target.value)
                          }
                          autoComplete="name"
                          placeholder={t("Not configured")}
                        />
                      </label>
                      <label>
                        <span>{t("Email")}</span>
                        <input
                          type="email"
                          value={globalIdentityEmail}
                          onChange={(event) =>
                            setGlobalIdentityEmail(event.target.value)
                          }
                          autoComplete="email"
                          placeholder={t("Not configured")}
                        />
                      </label>
                      <div className="identity-card-actions">
                        <small>{t("Leave a field empty to remove its global value.")}</small>
                        <button
                          type="submit"
                          disabled={Boolean(identitySaving)}
                        >
                          {identitySaving === "global"
                            ? t("Saving…")
                            : t("Save global identity")}
                        </button>
                      </div>
                    </form>
                  </>
                )}
                {identityError && (
                  <p className="identity-feedback error" role="alert">
                    {identityError}
                  </p>
                )}
                {identityMessage && (
                  <p className="identity-feedback success" role="status">
                    {identityMessage}
                  </p>
                )}
              </div>
              <div className="settings-section">
                <h3>{t("Appearance")}</h3>
                <p className="settings-section-desc">{t("Select theme mode")}</p>
                <div className="theme-options">
                  <button
                    type="button"
                    className={`theme-option-card ${themeSetting === "system" ? "selected" : ""}`}
                    onClick={() => setThemeSetting("system")}
                  >
                    <span className="theme-icon" aria-hidden="true">💻</span>
                    <span className="theme-label">{t("System")}</span>
                    <span className="theme-desc">{t("System default")}</span>
                  </button>
                  <button
                    type="button"
                    className={`theme-option-card ${themeSetting === "light" ? "selected" : ""}`}
                    onClick={() => setThemeSetting("light")}
                  >
                    <span className="theme-icon" aria-hidden="true">☀️</span>
                    <span className="theme-label">{t("Light")}</span>
                  </button>
                  <button
                    type="button"
                    className={`theme-option-card ${themeSetting === "dark" ? "selected" : ""}`}
                    onClick={() => setThemeSetting("dark")}
                  >
                    <span className="theme-icon" aria-hidden="true">🌙</span>
                    <span className="theme-label">{t("Dark")}</span>
                  </button>
                </div>
              </div>
              <div className="settings-section">
                <h3>{t("Commit history")}</h3>
                <label className="settings-toggle-row">
                  <span>
                    <strong>{t("Show Gravatar images")}</strong>
                    <small>
                      {t("Loads author images from Gravatar using hashed email addresses.")}
                    </small>
                  </span>
                  <input
                    type="checkbox"
                    checked={showGravatars}
                    onChange={(event) => setShowGravatars(event.target.checked)}
                  />
                </label>
              </div>
            </div>
          </div>
        </div>
      )}
      {showRepositorySettings && (
        <div
          className="modal-overlay"
          onClick={() => setShowRepositorySettings(false)}
          role="presentation"
        >
          <div
            className="settings-modal repository-settings-modal"
            onClick={(event) => event.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-labelledby="repository-settings-title"
          >
            <div className="settings-modal-header">
              <div>
                <h2 id="repository-settings-title">
                  {t("Repository settings")}
                </h2>
                {repositoryIdentity && (
                  <small>{repositoryIdentity.repositoryName}</small>
                )}
              </div>
              <button
                className="settings-close-btn"
                type="button"
                aria-label={t("Close repository settings")}
                onClick={() => setShowRepositorySettings(false)}
              >
                ×
              </button>
            </div>
            <div className="settings-modal-body">
              <div className="settings-section git-identity-settings">
                <h3>{t("Git author identity")}</h3>
                <p className="settings-section-desc">
                  {t("Override the global Git identity for this repository.")}
                </p>
                {identityLoading ? (
                  <p className="identity-state">{t("Loading Git identity…")}</p>
                ) : repositoryIdentity ? (
                  <form
                    className="identity-card"
                    aria-label={t("Repository Git identity")}
                    onSubmit={handleSaveRepositoryIdentity}
                  >
                    <div className="identity-card-heading">
                      <div>
                        <strong>{t("Current repository")}</strong>
                        <small>{repositoryIdentity.repositoryName}</small>
                      </div>
                      <span className="identity-scope-badge">--local</span>
                    </div>
                    <label>
                      <span className="identity-field-heading">
                        <span>{t("User name")}</span>
                        <span className="identity-override-toggle">
                          <input
                            type="checkbox"
                            checked={overrideRepositoryName}
                            aria-label={t("Override name for this repository")}
                            onChange={(event) => {
                              const checked = event.target.checked;
                              setOverrideRepositoryName(checked);
                              if (
                                checked &&
                                !repositoryIdentityName.trim()
                              ) {
                                setRepositoryIdentityName(
                                  repositoryIdentity.effective.name ??
                                    globalIdentityName,
                                );
                              }
                            }}
                          />
                          {t("Override")}
                        </span>
                      </span>
                      <input
                        value={repositoryIdentityName}
                        disabled={!overrideRepositoryName}
                        onChange={(event) =>
                          setRepositoryIdentityName(event.target.value)
                        }
                        autoComplete="name"
                        placeholder={t("Inherited from Git configuration")}
                      />
                    </label>
                    <label>
                      <span className="identity-field-heading">
                        <span>{t("Email")}</span>
                        <span className="identity-override-toggle">
                          <input
                            type="checkbox"
                            checked={overrideRepositoryEmail}
                            aria-label={t("Override email for this repository")}
                            onChange={(event) => {
                              const checked = event.target.checked;
                              setOverrideRepositoryEmail(checked);
                              if (
                                checked &&
                                !repositoryIdentityEmail.trim()
                              ) {
                                setRepositoryIdentityEmail(
                                  repositoryIdentity.effective.email ??
                                    globalIdentityEmail,
                                );
                              }
                            }}
                          />
                          {t("Override")}
                        </span>
                      </span>
                      <input
                        type="email"
                        value={repositoryIdentityEmail}
                        disabled={!overrideRepositoryEmail}
                        onChange={(event) =>
                          setRepositoryIdentityEmail(event.target.value)
                        }
                        autoComplete="email"
                        placeholder={t("Inherited from Git configuration")}
                      />
                    </label>
                    <div className="identity-card-actions">
                      <small>
                        {t("Turn off an override to use the inherited value.")}
                      </small>
                      <button
                        type="submit"
                        disabled={Boolean(identitySaving)}
                      >
                        {identitySaving === "repository"
                          ? t("Saving…")
                          : t("Save repository identity")}
                      </button>
                    </div>
                  </form>
                ) : (
                  <div className="identity-empty">
                    <strong>{t("Repository unavailable")}</strong>
                    <small>
                      {t("The selected repository is no longer open.")}
                    </small>
                  </div>
                )}
                {identityError && (
                  <p className="identity-feedback error" role="alert">
                    {identityError}
                  </p>
                )}
                {identityMessage && (
                  <p className="identity-feedback success" role="status">
                    {identityMessage}
                  </p>
                )}
              </div>
            </div>
          </div>
        </div>
      )}
      {submoduleContextMenu && activeSnapshot && (
        <div
          className="remote-context-menu"
          role="menu"
          style={{ left: submoduleContextMenu.x, top: submoduleContextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          {submoduleContextMenu.submodule.initialized ? (
            <>
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  const submodule = submoduleContextMenu.submodule;
                  setSubmoduleContextMenu(undefined);
                  void handleSubmoduleOpen(submodule);
                }}
              >
                {t("Open")}
              </button>
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  const submodule = submoduleContextMenu.submodule;
                  setSubmoduleContextMenu(undefined);
                  void handleSubmoduleDeinitialize(submodule);
                }}
              >
                {t("Deinitialize")}
              </button>
            </>
          ) : (
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                const path = submoduleContextMenu.submodule.path;
                setSubmoduleContextMenu(undefined);
                handleSubmoduleInitialize(path);
              }}
            >
              {t("Initialize")}
            </button>
          )}
        </div>
      )}
      {referenceContextMenu && activeSnapshot && (
        <div
          className="remote-context-menu"
          role="menu"
          style={{ left: referenceContextMenu.x, top: referenceContextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          {referenceContextMenu.kind === "branch" ? (
            <>
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setReferenceEditor({
                    mode: "createBranch",
                    source: referenceContextMenu.name,
                  });
                  setReferenceContextMenu(undefined);
                }}
              >
                {t("New branch from here")}
              </button>
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setReferenceEditor({
                    mode: "renameBranch",
                    name: referenceContextMenu.name,
                    upstream: referenceContextMenu.upstream,
                  });
                  setReferenceContextMenu(undefined);
                }}
              >
                {t("Rename branch")}
              </button>
              <button
                type="button"
                role="menuitem"
                className="danger-button"
                disabled={
                  activeSnapshot.head.kind === "branch" &&
                  activeSnapshot.head.name === referenceContextMenu.name
                }
                onClick={() => {
                  const name = referenceContextMenu.name;
                  const upstream = referenceContextMenu.upstream;
                  const repoId = activeSnapshot.repository.id;
                  setReferenceContextMenu(undefined);
                  openBranchDeleteDialog(repoId, name, upstream);
                }}
              >
                {t("Delete branch")}
              </button>
              <button
                type="button"
                role="menuitem"
                disabled={
                  activeSnapshot.head.kind !== "branch" ||
                  activeSnapshot.head.name === referenceContextMenu.name
                }
                onClick={() => {
                  const name = referenceContextMenu.name;
                  const current =
                    activeSnapshot.head.kind === "branch"
                      ? (activeSnapshot.head.name ?? "HEAD")
                      : "HEAD";
                  setReferenceContextMenu(undefined);
                  if (
                    confirmRepositoryMutation(
                      t("Rebase {branch} onto {target}?", {
                        branch: current,
                        target: name,
                      }),
                    )
                  ) {
                    void handleWorkspaceMutation(() =>
                      rebaseBranch(
                        activeSnapshot.repository.id,
                        activeSnapshot.revision,
                        name,
                      ),
                    );
                  }
                }}
              >
                {t("Rebase current branch onto this branch")}
              </button>
              <button
                type="button"
                role="menuitem"
                disabled={activeSnapshot.head.kind !== "branch"}
                onClick={() => {
                  const branch = referenceContextMenu.name;
                  setReferenceContextMenu(undefined);
                  void handleWorkspaceMutation(() =>
                    fastForwardBranch(
                      activeSnapshot.repository.id,
                      activeSnapshot.revision,
                      branch,
                    ),
                  );
                }}
              >
                {t("Fast-forward to {branch}", {
                  branch: referenceContextMenu.name,
                })}
              </button>
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setReferenceEditor({
                    mode: "createTag",
                    target: referenceContextMenu.name,
                  });
                  setReferenceContextMenu(undefined);
                }}
              >
                {t("Create tag here")}
              </button>
            </>
          ) : (
            <button
              type="button"
              role="menuitem"
              className="danger-button"
              onClick={() => {
                const name = referenceContextMenu.name;
                const repoId = activeSnapshot.repository.id;
                setReferenceContextMenu(undefined);
                void openTagDeleteDialog(repoId, name);
              }}
            >
              {t("Delete tag")}
            </button>
          )}
        </div>
      )}
      {stashContextMenu && activeSnapshot && (
        <div
          className="remote-context-menu"
          role="menu"
          style={{ left: stashContextMenu.x, top: stashContextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setStashDialog(stashContextMenu.stash);
              setStashContextMenu(undefined);
            }}
          >
            {t("Apply")}
          </button>
          <button
            type="button"
            role="menuitem"
            className="danger-button"
            onClick={() => {
              const { reference } = stashContextMenu.stash;
              setStashContextMenu(undefined);
              if (
                confirmRepositoryMutation(
                  t("Drop {reference}? The stash entry cannot be recovered.", {
                    reference,
                  }),
                )
              ) {
                void handleWorkspaceMutation(() =>
                  dropStash(
                    activeSnapshot.repository.id,
                    activeSnapshot.revision,
                    reference,
                  ),
                );
              }
            }}
          >
            {t("Drop")}
          </button>
        </div>
      )}
      {remoteContextMenu && activeSnapshot && (
        <div
          className="remote-context-menu"
          role="menu"
          style={{ left: remoteContextMenu.x, top: remoteContextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          {remoteContextMenu.remote ? (
            <>
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setRemoteEditor({
                    mode: "edit",
                    remote: remoteContextMenu.remote,
                  });
                  setRemoteContextMenu(undefined);
                }}
              >
                {t("Edit remote")}
              </button>
              <button
                type="button"
                role="menuitem"
                disabled={loadingRemoteTags}
                onClick={() => {
                  handleFetchRemoteTags(remoteContextMenu.remote?.name);
                  setRemoteContextMenu(undefined);
                }}
              >
                {t("Refresh remote tags")}
              </button>
              <button
                type="button"
                role="menuitem"
                className="danger-button"
                onClick={() => {
                  const name = remoteContextMenu.remote?.name;
                  setRemoteContextMenu(undefined);
                  if (
                    name &&
                    confirmRepositoryMutation(
                      t("Remove remote {name}? Remote-tracking branches will be deleted.", {
                        name,
                      }),
                    )
                  ) {
                    void handleRemoteMutation(() =>
                      removeRemote(
                        activeSnapshot.repository.id,
                        activeSnapshot.revision,
                        name,
                      ),
                    ).catch(() => undefined);
                  }
                }}
              >
                {t("Remove remote")}
              </button>
            </>
          ) : (
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setRemoteEditor({ mode: "add" });
                setRemoteContextMenu(undefined);
              }}
            >
              {t("Add remote")}
            </button>
          )}
        </div>
      )}
      {remoteEditor && activeSnapshot && (
        <RemoteEditor
          mode={remoteEditor.mode}
          remote={remoteEditor.remote}
          onClose={() => setRemoteEditor(undefined)}
          onSave={(name, url) =>
            remoteEditor.mode === "edit" && remoteEditor.remote
              ? handleRemoteMutation(() =>
                  updateRemote(
                    activeSnapshot.repository.id,
                    activeSnapshot.revision,
                    remoteEditor.remote!.name,
                    { name, url },
                  ),
                )
              : handleRemoteMutation(() =>
                  addRemote(
                    activeSnapshot.repository.id,
                    activeSnapshot.revision,
                    { name, url },
                  ),
                )
          }
        />
      )}
      {showSubmoduleAdd && activeSnapshot && (
        <SubmoduleAddDialog
          onClose={() => setShowSubmoduleAdd(false)}
          onAdd={(url, path) =>
            handleWorkspaceMutation(() =>
              addSubmodule(
                activeSnapshot.repository.id,
                activeSnapshot.revision,
                { url, path },
              ),
            )
          }
        />
      )}
      {remoteDialog && activeSnapshot && (
        <RemoteOperationDialog
          kind={remoteDialog}
          remotes={remotes}
          onClose={() => setRemoteDialog(undefined)}
          onRun={(options) => {
            handleRemote(remoteDialog, options);
            setRemoteDialog(undefined);
          }}
        />
      )}
      {stashDialog && activeSnapshot && (
        <StashApplyDialog
          stash={stashDialog}
          onClose={() => setStashDialog(undefined)}
          onApply={(dropAfterApply) =>
            handleStashApply(stashDialog.reference, dropAfterApply)
          }
        />
      )}
      {referenceEditor && activeSnapshot && (
        <ReferenceEditorDialog
          editor={referenceEditor}
          blocked={refreshing.has(activeSnapshot.repository.id)}
          onClose={() => setReferenceEditor(undefined)}
          onSave={(name, renameRemote) => {
            if (referenceEditor.mode === "createBranch") {
              return handleWorkspaceMutation(() =>
                createBranch(
                  activeSnapshot.repository.id,
                  activeSnapshot.revision,
                  { name, startPoint: referenceEditor.source },
                ),
              );
            }
            if (referenceEditor.mode === "renameBranch") {
              return handleWorkspaceMutation(() =>
                renameBranch(
                  activeSnapshot.repository.id,
                  activeSnapshot.revision,
                  referenceEditor.name,
                  name,
                  renameRemote,
                ),
              );
            }
            return handleWorkspaceMutation(() =>
              createTag(
                activeSnapshot.repository.id,
                activeSnapshot.revision,
                name,
                referenceEditor.target,
              ),
            );
          }}
        />
      )}
      {referenceDeleteDialog && (
        <ReferenceDeleteDialog
          request={referenceDeleteDialog}
          onClose={() => setReferenceDeleteDialog(undefined)}
          onDelete={(deleteRemote) =>
            handleReferenceDelete(referenceDeleteDialog, deleteRemote)
          }
        />
      )}
      {checkoutTarget && activeSnapshot && (
        <CheckoutDialog
          target={checkoutTarget}
          hasChanges={activeSnapshot.changes.length > 0}
          onClose={() => setCheckoutTarget(undefined)}
          onCheckout={(autoStash) =>
            handleWorkspaceMutation(() =>
              checkoutBranch(
                activeSnapshot.repository.id,
                activeSnapshot.revision,
                checkoutTarget.name,
                checkoutTarget.kind === "remoteBranch",
                checkoutTarget.kind === "tag",
                autoStash,
              ),
            )
          }
        />
      )}
    </div>
  );
}

function SubmoduleAddDialog({
  onClose,
  onAdd,
}: {
  onClose: () => void;
  onAdd: (url: string, path: string) => Promise<void>;
}) {
  const [url, setUrl] = useState("");
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);

  return (
    <div className="modal-overlay" onClick={onClose} role="presentation">
      <div
        className="settings-modal remote-manager-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="submodule-add-title"
      >
        <div className="settings-modal-header">
          <h2 id="submodule-add-title">{t("Add submodule")}</h2>
          <button
            className="settings-close-btn"
            type="button"
            aria-label={t("Close add submodule dialog")}
            onClick={onClose}
          >
            ×
          </button>
        </div>
        <div className="remote-manager-body">
          <form
            className="remote-form"
            onSubmit={(event) => {
              event.preventDefault();
              setBusy(true);
              void onAdd(url.trim(), path.trim())
                .then(onClose)
                .catch(() => undefined)
                .finally(() => setBusy(false));
            }}
          >
            <label>
              <span>{t("Repository URL")}</span>
              <input
                value={url}
                onChange={(event) => setUrl(event.target.value)}
                disabled={busy}
                autoFocus
                placeholder="https://host/owner/repository.git"
              />
            </label>
            <label>
              <span>{t("Path in repository")}</span>
              <input
                value={path}
                onChange={(event) => setPath(event.target.value)}
                disabled={busy}
                placeholder="vendor/repository"
              />
            </label>
            <small>
              {t("The submodule and .gitmodules changes will be staged.")}
            </small>
            <div className="remote-form-actions">
              <button type="button" disabled={busy} onClick={onClose}>
                {t("Cancel")}
              </button>
              <button
                type="submit"
                disabled={busy || !url.trim() || !path.trim()}
              >
                {busy ? t("Adding…") : t("Add submodule")}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}

function RemoteEditor({
  mode,
  remote,
  onClose,
  onSave,
}: {
  mode: "add" | "edit";
  remote?: GitRemoteDto;
  onClose: () => void;
  onSave: (name: string, url: string) => Promise<void>;
}) {
  const [name, setName] = useState(remote?.name ?? "");
  const [url, setUrl] = useState(remote?.url ?? "");
  const [busy, setBusy] = useState(false);

  const save = async () => {
    setBusy(true);
    try {
      await onSave(name.trim(), url.trim());
      onClose();
    } catch {
      // The parent displays normalized command errors in the app error banner.
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose} role="presentation">
      <div
        className="settings-modal remote-manager-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="remote-manager-title"
      >
        <div className="settings-modal-header">
          <h2 id="remote-manager-title">
            {mode === "edit" ? t("Edit remote") : t("Add remote")}
          </h2>
          <button
            className="settings-close-btn"
            type="button"
            aria-label={t("Close remote manager")}
            onClick={onClose}
          >
            ×
          </button>
        </div>
        <div className="remote-manager-body">
          <form
            className="remote-form"
            onSubmit={(event) => {
              event.preventDefault();
              const nextName = name.trim();
              const nextUrl = url.trim();
              if (!nextName || !nextUrl) return;
              void save();
            }}
          >
            <label>
              <span>{t("Remote name")}</span>
              <input
                value={name}
                onChange={(event) => setName(event.target.value)}
                disabled={busy}
                placeholder="origin"
                autoFocus
              />
            </label>
            <label>
              <span>{t("Remote URL")}</span>
              <input
                value={url}
                onChange={(event) => setUrl(event.target.value)}
                disabled={busy}
                placeholder="https://example.com/owner/repository.git"
              />
            </label>
            <div className="remote-form-actions">
              <button type="button" disabled={busy} onClick={onClose}>
                {t("Cancel")}
              </button>
              <button type="submit" disabled={busy || !name.trim() || !url.trim()}>
                {busy
                  ? t("Saving…")
                  : mode === "edit"
                    ? t("Save remote")
                    : t("Add remote")}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}

function RemoteOperationDialog({
  kind,
  remotes,
  onClose,
  onRun,
}: {
  kind: "fetch" | "pull" | "push";
  remotes: GitRemoteDto[];
  onClose: () => void;
  onRun: (options: RemoteOperationOptions) => void;
}) {
  const [remote, setRemote] = useState(
    remotes.find((item) => item.name === "origin")?.name ?? remotes[0]?.name ?? "",
  );
  const [fetchTags, setFetchTags] = useState(false);
  const [autoStash, setAutoStash] = useState(false);
  const [fastForwardOnly, setFastForwardOnly] = useState(true);
  const [forceWithLease, setForceWithLease] = useState(false);
  const title = t(kind === "fetch" ? "Fetch" : kind === "pull" ? "Pull" : "Push");

  useEffect(() => {
    setRemote((current) =>
      remotes.some((item) => item.name === current)
        ? current
        : (remotes.find((item) => item.name === "origin")?.name ?? remotes[0]?.name ?? ""),
    );
  }, [remotes]);

  return (
    <div className="modal-overlay" onClick={onClose} role="presentation">
      <div
        className="settings-modal remote-operation-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="remote-operation-title"
      >
        <div className="settings-modal-header">
          <h2 id="remote-operation-title">{title}</h2>
          <button
            className="settings-close-btn"
            type="button"
            aria-label={t("Close remote operation")}
            onClick={onClose}
          >
            ×
          </button>
        </div>
        <div className="remote-manager-body">
          <form
            className="remote-form remote-operation-form"
            onSubmit={(event) => {
              event.preventDefault();
              if (!remote) return;
              onRun({
                remote,
                fetchTags: kind === "fetch" && fetchTags,
                autoStash: kind === "pull" && autoStash,
                fastForwardOnly: kind === "pull" && fastForwardOnly,
                forceWithLease: kind === "push" && forceWithLease,
              });
            }}
          >
            <label>
              <span>{t("Remote")}</span>
              <select
                value={remote}
                onChange={(event) => setRemote(event.target.value)}
                autoFocus
              >
                {remotes.map((item) => (
                  <option key={item.name} value={item.name}>
                    {item.name}
                  </option>
                ))}
              </select>
            </label>
            {remotes.length === 0 && (
              <small>{t("No remotes configured.")}</small>
            )}
            {kind === "fetch" && (
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={fetchTags}
                  onChange={(event) => setFetchTags(event.target.checked)}
                />
                <span>{t("Fetch tags")}</span>
              </label>
            )}
            {kind === "pull" && (
              <>
                <label className="checkbox-row">
                  <input
                    type="checkbox"
                    checked={autoStash}
                    onChange={(event) => setAutoStash(event.target.checked)}
                  />
                  <span>{t("Automatically stash and reapply local changes")}</span>
                </label>
                <label className="checkbox-row">
                  <input
                    type="checkbox"
                    checked={fastForwardOnly}
                    onChange={(event) => setFastForwardOnly(event.target.checked)}
                  />
                  <span>{t("Use fast-forward only")}</span>
                </label>
              </>
            )}
            {kind === "push" && (
              <>
                <label className="checkbox-row">
                  <input
                    type="checkbox"
                    checked={forceWithLease}
                    onChange={(event) => setForceWithLease(event.target.checked)}
                  />
                  <span>{t("Force Push")}</span>
                </label>
                {forceWithLease && (
                  <small>
                    {t("Reject if the remote changed since the last fetch")}
                  </small>
                )}
              </>
            )}
            <div className="remote-form-actions">
              <button type="button" onClick={onClose}>
                {t("Cancel")}
              </button>
              <button type="submit" disabled={!remote}>
                {title}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}

function StashApplyDialog({
  stash,
  onClose,
  onApply,
}: {
  stash: StashItem;
  onClose: () => void;
  onApply: (dropAfterApply: boolean) => Promise<boolean>;
}) {
  const [dropAfterApply, setDropAfterApply] = useState(false);
  const [busy, setBusy] = useState(false);

  return (
    <div
      className="modal-overlay"
      onClick={() => !busy && onClose()}
      role="presentation"
    >
      <div
        className="settings-modal remote-operation-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="stash-apply-title"
      >
        <div className="settings-modal-header">
          <h2 id="stash-apply-title">{t("Apply stash")}</h2>
          <button
            className="settings-close-btn"
            type="button"
            aria-label={t("Close stash operation")}
            disabled={busy}
            onClick={onClose}
          >
            ×
          </button>
        </div>
        <div className="remote-manager-body">
          <form
            className="remote-form remote-operation-form"
            onSubmit={(event) => {
              event.preventDefault();
              setBusy(true);
              void onApply(dropAfterApply).then((succeeded) => {
                setBusy(false);
                if (succeeded) onClose();
              });
            }}
          >
            <div className="stash-dialog-summary">
              <code>{stash.reference}</code>
              <span>{stash.message}</span>
            </div>
            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={dropAfterApply}
                disabled={busy}
                onChange={(event) => setDropAfterApply(event.target.checked)}
              />
              <span>{t("Drop this stash after applying")}</span>
            </label>
            <div className="remote-form-actions">
              <button type="button" disabled={busy} onClick={onClose}>
                {t("Cancel")}
              </button>
              <button type="submit" disabled={busy} autoFocus>
                {busy ? t("Applying…") : t("Apply")}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}

function StashCreateDialog({
  count,
  busy,
  onClose,
  onCreate,
}: {
  count: number;
  busy: boolean;
  onClose: () => void;
  onCreate: (name: string) => void;
}) {
  const [name, setName] = useState("");

  return (
    <div
      className="modal-overlay"
      onClick={() => !busy && onClose()}
      role="presentation"
    >
      <div
        className="settings-modal remote-operation-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="stash-create-title"
      >
        <div className="settings-modal-header">
          <h2 id="stash-create-title">{t("Create stash")}</h2>
          <button
            className="settings-close-btn"
            type="button"
            aria-label={t("Close stash creation")}
            disabled={busy}
            onClick={onClose}
          >
            ×
          </button>
        </div>
        <div className="remote-manager-body">
          <form
            className="remote-form remote-operation-form"
            onSubmit={(event) => {
              event.preventDefault();
              onCreate(name);
            }}
          >
            <p>
              {t("{count} selected files will be stashed.", { count })}
            </p>
            <label>
              <span>{t("Stash name")}</span>
              <input
                value={name}
                disabled={busy}
                autoFocus
                onChange={(event) => setName(event.target.value)}
                placeholder={t("Stash name (optional)")}
              />
            </label>
            <div className="remote-form-actions">
              <button type="button" disabled={busy} onClick={onClose}>
                {t("Cancel")}
              </button>
              <button type="submit" disabled={busy}>
                {busy ? t("Stashing selected files…") : t("Create stash")}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}

function ReferenceDeleteDialog({
  request,
  onClose,
  onDelete,
}: {
  request: ReferenceDeleteDialogState;
  onClose: () => void;
  onDelete: (deleteRemote: boolean) => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const title =
    request.kind === "branch" ? t("Delete branch") : t("Delete tag");
  const remoteNames = request.remoteReferences
    .map((reference) => `${reference.remote}/${reference.name}`)
    .join(", ");
  const remove = async (deleteRemote: boolean) => {
    setBusy(true);
    await onDelete(deleteRemote);
    setBusy(false);
    onClose();
  };

  return (
    <div
      className="modal-overlay"
      onClick={() => !busy && onClose()}
      role="presentation"
    >
      <div
        className="settings-modal remote-operation-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="reference-delete-title"
      >
        <div className="settings-modal-header">
          <h2 id="reference-delete-title">{title}</h2>
          <button
            className="settings-close-btn"
            type="button"
            aria-label={t("Cancel")}
            disabled={busy}
            onClick={onClose}
          >
            ×
          </button>
        </div>
        <div className="remote-manager-body">
          <form
            className="remote-form remote-operation-form"
            onSubmit={(event) => event.preventDefault()}
          >
            <p>
              {request.kind === "branch"
                ? t("Delete merged branch {branch}?", { branch: request.name })
                : t("Delete tag {tag}?", { tag: request.name })}
            </p>
            {request.loading ? (
              <small>{t("Checking remotes…")}</small>
            ) : request.remoteReferences.length > 0 ? (
              <small>
                {t("This reference also exists on {remotes}.", {
                  remotes: remoteNames,
                })}
              </small>
            ) : (
              <small>{t("No matching remote reference was found.")}</small>
            )}
            <div className="remote-form-actions">
              <button type="button" disabled={busy} onClick={onClose}>
                {t("Cancel")}
              </button>
              <button
                type="button"
                className="danger-button"
                disabled={busy || request.loading}
                onClick={() => void remove(false)}
                autoFocus
              >
                {busy ? t("Deleting…") : t("Delete locally")}
              </button>
              {request.remoteReferences.length > 0 && (
                <button
                  type="button"
                  className="danger-button"
                  disabled={busy || request.loading}
                  onClick={() => void remove(true)}
                >
                  {busy ? t("Deleting…") : t("Delete locally and remotely")}
                </button>
              )}
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}

function ReferenceEditorDialog({
  editor,
  blocked,
  onClose,
  onSave,
}: {
  editor: ReferenceEditor;
  blocked: boolean;
  onClose: () => void;
  onSave: (name: string, renameRemote: boolean) => Promise<void>;
}) {
  const [name, setName] = useState(
    editor.mode === "renameBranch" ? editor.name : "",
  );
  const [renameRemote, setRenameRemote] = useState(false);
  const [busy, setBusy] = useState(false);
  const title =
    editor.mode === "createBranch"
      ? t("Create branch")
      : editor.mode === "renameBranch"
        ? t("Rename branch")
        : t("Create tag");
  const fieldLabel =
    editor.mode === "createTag" ? t("Tag name") : t("Branch name");

  return (
    <div className="modal-overlay" onClick={onClose} role="presentation">
      <div
        className="settings-modal remote-manager-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="reference-editor-title"
      >
        <div className="settings-modal-header">
          <h2 id="reference-editor-title">{title}</h2>
          <button
            className="settings-close-btn"
            type="button"
            aria-label={t("Close reference editor")}
            onClick={onClose}
          >
            ×
          </button>
        </div>
        <div className="remote-manager-body">
          <form
            className="remote-form"
            onSubmit={(event) => {
              event.preventDefault();
              const nextName = name.trim();
              if (!nextName || blocked) return;
              setBusy(true);
              void onSave(nextName, renameRemote).finally(() => {
                setBusy(false);
                onClose();
              });
            }}
          >
            <label>
              <span>{fieldLabel}</span>
              <input
                value={name}
                onChange={(event) => setName(event.target.value)}
                disabled={busy}
                autoFocus
              />
            </label>
            {editor.mode === "createBranch" && (
              <small>
                {t("The new branch will start at {branch}.", {
                  branch: editor.source,
                })}
              </small>
            )}
            {editor.mode === "createTag" && (
              <small>
                {t("The tag will point to {branch}.", {
                  branch: editor.target,
                })}
              </small>
            )}
            {editor.mode === "renameBranch" && editor.upstream && (
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={renameRemote}
                  onChange={(event) => setRenameRemote(event.target.checked)}
                  disabled={busy}
                />
                <span>
                  {t("Also rename upstream branch {upstream}", {
                    upstream: editor.upstream,
                  })}
                </span>
              </label>
            )}
            <div className="remote-form-actions">
              <button type="button" disabled={busy} onClick={onClose}>
                {t("Cancel")}
              </button>
              <button type="submit" disabled={busy || blocked || !name.trim()}>
                {busy ? t("Saving…") : title}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}

function CheckoutDialog({
  target,
  hasChanges,
  onClose,
  onCheckout,
}: {
  target: CheckoutTarget;
  hasChanges: boolean;
  onClose: () => void;
  onCheckout: (autoStash: boolean) => Promise<void>;
}) {
  const [autoStash, setAutoStash] = useState(hasChanges);
  const [busy, setBusy] = useState(false);

  return (
    <div className="modal-overlay" onClick={onClose} role="presentation">
      <div
        className="settings-modal remote-manager-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="checkout-dialog-title"
      >
        <div className="settings-modal-header">
          <h2 id="checkout-dialog-title">{t("Checkout branch")}</h2>
          <button
            className="settings-close-btn"
            type="button"
            aria-label={t("Close checkout dialog")}
            onClick={onClose}
          >
            ×
          </button>
        </div>
        <div className="remote-manager-body">
          <form
            className="remote-form"
            onSubmit={(event) => {
              event.preventDefault();
              setBusy(true);
              void onCheckout(autoStash).finally(() => {
                setBusy(false);
                onClose();
              });
            }}
          >
            <p>
              {target.kind === "remoteBranch"
                ? t("Check out remote branch {branch} as a tracking branch?", {
                    branch: target.name,
                  })
                : target.kind === "tag"
                  ? t("Check out tag {tag} in detached HEAD mode?", {
                      tag: target.name,
                    })
                  : t("Check out branch {branch}?", { branch: target.name })}
            </p>
            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={autoStash}
                onChange={(event) => setAutoStash(event.target.checked)}
                disabled={busy}
              />
              <span>{t("Automatically stash and reapply local changes")}</span>
            </label>
            <small>
              {t(
                "Includes untracked files and restores staged changes after checkout.",
              )}
            </small>
            <div className="remote-form-actions">
              <button type="button" disabled={busy} onClick={onClose}>
                {t("Cancel")}
              </button>
              <button type="submit" disabled={busy}>
                {busy ? t("Checking out…") : t("Checkout")}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}

function repositoryName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? "Repository";
}

function cloneRepositoryName(remoteUrl: string) {
  const withoutQuery = remoteUrl.split(/[?#]/, 1)[0].replace(/[\\/]+$/, "");
  const name = withoutQuery.split(/[\\/:]/).filter(Boolean).at(-1) ?? "repository";
  return name.replace(/\.git$/i, "") || "repository";
}

function focusSelectionIndex(element: HTMLElement, index: number) {
  const scope = element.dataset.selectionScope;
  if (!scope) return;
  requestAnimationFrame(() => {
    document
      .querySelector<HTMLElement>(
        `[data-selection-scope="${scope}"][data-selection-index="${index}"]`,
      )
      ?.focus();
  });
}

export type BranchTreeNode = {
  id: string;
  name: string;
  fullPath: string;
  isRemoteRoot: boolean;
  isFolder: boolean;
  isLeaf: boolean;
  count: number;
  children: BranchTreeNode[];
};

type BranchTreeBuilderNode = {
  name: string;
  path: string;
  isRemoteRoot: boolean;
  childrenMap: Map<string, BranchTreeBuilderNode>;
  count: number;
};

export function buildBranchTree(
  branchNames: string[],
  isRemote = false,
): BranchTreeNode[] {
  const rootMap = new Map<string, BranchTreeBuilderNode>();

  for (const name of branchNames) {
    if (!name) continue;
    const parts = name.split("/");
    let currentLevel = rootMap;

    let pathSoFar = "";
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      pathSoFar = pathSoFar ? `${pathSoFar}/${part}` : part;
      const isRemoteRoot = isRemote && i === 0;

      if (!currentLevel.has(part)) {
        currentLevel.set(part, {
          name: part,
          path: pathSoFar,
          isRemoteRoot,
          childrenMap: new Map(),
          count: 0,
        });
      }

      const node = currentLevel.get(part)!;
      node.count += 1;
      currentLevel = node.childrenMap;
    }
  }

  function convertMap(
    map: Map<string, BranchTreeBuilderNode>,
  ): BranchTreeNode[] {
    const list: BranchTreeNode[] = [];
    for (const [, val] of map.entries()) {
      const children = convertMap(val.childrenMap);
      const isFolder = children.length > 0;
      const isLeaf = !isFolder;

      list.push({
        id: val.path,
        name: val.name,
        fullPath: val.path,
        isRemoteRoot: val.isRemoteRoot,
        isFolder,
        isLeaf,
        count: val.count,
        children,
      });
    }
    return list;
  }

  return convertMap(rootMap);
}

export function buildRemoteBranchTree(branchNames: string[]) {
  return buildBranchTree(branchNames, true);
}

function BranchTreeNodeView({
  node,
  depth = 0,
  currentBranchLabel,
  referencesList = [],
  isRemote = false,
  selection,
  selectionPrefix = "",
  pathPrefix = "",
  onSelectSelectionKey,
  onCheckout,
  onSelect,
  onContextMenu,
}: {
  node: BranchTreeNode;
  depth?: number;
  currentBranchLabel?: string;
  referencesList?: ReferenceDto[];
  isRemote?: boolean;
  selection?: MultiSelection;
  selectionPrefix?: string;
  pathPrefix?: string;
  onSelectSelectionKey?: (selectionKey: string) => void;
  onCheckout?: (branchName: string) => void;
  onSelect?: (fullPath: string) => void;
  onContextMenu?: (
    event: ReactMouseEvent<HTMLElement>,
    branchName: string,
  ) => void;
}) {
  const [isExpanded, setIsExpanded] = useState(true);

  if (node.isLeaf) {
    const referencePath = pathPrefix
      ? `${pathPrefix}/${node.fullPath}`
      : node.fullPath;
    const selectionKey = `${selectionPrefix}${referencePath}`;
    const selectionIndex = selection?.items.indexOf(selectionKey);
    const isCurrent = !isRemote && referencePath === currentBranchLabel;
    const refInfo = !isRemote
      ? referencesList.find(
          (r) => r.kind === "localBranch" && r.shortName === referencePath,
        )
      : undefined;
    const isLocalOnly = !isRemote && refInfo ? !refInfo.upstream : false;

    return (
      <div
        className={`tree-leaf-row branch-item-row ${isCurrent ? "active-branch" : ""} ${selection?.selected.has(selectionKey) ? "selected" : ""}`}
        role="button"
        tabIndex={
          selection
            ? selection.focused === selectionKey ||
              (!selection.focused && selectionIndex === 0)
              ? 0
              : -1
            : 0
        }
        aria-label={t("Branch {name}", { name: referencePath })}
        aria-pressed={selection?.selected.has(selectionKey)}
        data-selection-scope="branches"
        data-selection-index={selectionIndex}
        style={{ paddingLeft: `${depth * 14 + 6}px` }}
        onMouseDown={(event) => selection?.onMouseDown(selectionKey, event)}
        onMouseEnter={(event) => selection?.onMouseEnter(selectionKey, event)}
        onClick={(e) => {
          e.stopPropagation();
          selection?.onClick(selectionKey, e);
          if (onSelect) {
            onSelect(referencePath);
          } else if (!isRemote && onCheckout && !isCurrent) {
            onCheckout(referencePath);
          }
        }}
        onDoubleClick={(e) => {
          e.stopPropagation();
          if (onCheckout) {
            onCheckout(referencePath);
          }
        }}
        onContextMenu={(event) => onContextMenu?.(event, referencePath)}
        onKeyDown={(event) => {
          event.stopPropagation();
          const activate = (item = selectionKey) => {
            if (onSelectSelectionKey) {
              onSelectSelectionKey(item);
              return;
            }
            const fullPath = item.startsWith(selectionPrefix)
              ? item.slice(selectionPrefix.length)
              : referencePath;
            if (onSelect) {
              onSelect(fullPath);
            } else if (!isRemote && onCheckout && !isCurrent) {
              onCheckout(fullPath);
            }
          };
          if (selection) {
            selection.onKeyDown(
              selectionKey,
              event,
              activate,
              (index) => focusSelectionIndex(event.currentTarget, index),
            );
          } else if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            activate(selectionKey);
          }
        }}
      >
        <span className="branch-icon" aria-hidden="true">
          {isCurrent ? "● " : "⎇ "}
        </span>
        <span className="branch-name" title={referencePath}>
          {node.name}
        </span>
        {isLocalOnly && <span className="badge-local">{t("Local")}</span>}
        {refInfo && (refInfo.ahead > 0 || refInfo.behind > 0) && (
          <small className="track-counts">
            {refInfo.ahead > 0 ? `↑${refInfo.ahead}` : ""}
            {refInfo.behind > 0 ? `↓${refInfo.behind}` : ""}
          </small>
        )}
      </div>
    );
  }

  return (
    <div className="tree-node-group">
      <div
        className="tree-node-header"
        role="button"
        tabIndex={0}
        aria-expanded={isExpanded}
        style={{ paddingLeft: `${depth * 14 + 4}px` }}
        onClick={(e) => {
          e.stopPropagation();
          setIsExpanded((prev) => !prev);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.stopPropagation();
            e.preventDefault();
            setIsExpanded((prev) => !prev);
          }
        }}
      >
        <span className={`group-chevron ${isExpanded ? "open" : ""}`} aria-hidden="true">
          ›
        </span>
        <span className="tree-node-icon" aria-hidden="true">
          {node.isRemoteRoot ? "☁️ " : "📁 "}
        </span>
        <span className="tree-node-label">{node.name}</span>
        <small className="tree-node-count">({node.count})</small>
      </div>
      {isExpanded && (
        <div
          className="tree-node-children"
          style={{
            borderLeft: depth > 0 ? "1px solid rgba(255, 255, 255, 0.08)" : "none",
            marginLeft: `${depth * 14 + 8}px`,
          }}
        >
          {node.children.map((child) => (
            <BranchTreeNodeView
              key={child.id}
              node={child}
              depth={depth + 1}
              currentBranchLabel={currentBranchLabel}
              referencesList={referencesList}
              isRemote={isRemote}
              selection={selection}
              selectionPrefix={selectionPrefix}
              pathPrefix={pathPrefix}
              onSelectSelectionKey={onSelectSelectionKey}
              onCheckout={onCheckout}
              onSelect={onSelect}
              onContextMenu={onContextMenu}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function RemoteReferenceNode({
  name,
  branches,
  tags,
  branchSelection,
  tagSelection,
  tagSelectionItems,
  onBranchSelection,
  onTagSelection,
  onSelectBranch,
  onSelectTag,
  onCheckout,
  onContextMenu,
}: {
  name: string;
  branches: string[];
  tags: RemoteTagDto[];
  branchSelection: MultiSelection;
  tagSelection: MultiSelection;
  tagSelectionItems: string[];
  onBranchSelection: (selectionKey: string) => void;
  onTagSelection: (selectionKey: string) => void;
  onSelectBranch: (refName: string) => void;
  onSelectTag: (tag: RemoteTagDto) => void;
  onCheckout: (branchName: string) => void;
  onContextMenu: (event: ReactMouseEvent<HTMLDivElement>) => void;
}) {
  const [isExpanded, setIsExpanded] = useState(true);
  const branchTree = useMemo(() => buildBranchTree(branches), [branches]);

  return (
    <div className="tree-node-group remote-reference-node">
      <div
        className="tree-node-header remote-name-row"
        role="button"
        tabIndex={0}
        aria-expanded={isExpanded}
        aria-label={t("Remote {name}", { name })}
        onClick={(event) => {
          event.stopPropagation();
          setIsExpanded((expanded) => !expanded);
        }}
        onContextMenu={onContextMenu}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            setIsExpanded((expanded) => !expanded);
          }
        }}
      >
        <span className={`group-chevron ${isExpanded ? "open" : ""}`} aria-hidden="true">
          ›
        </span>
        <span className="tree-node-icon" aria-hidden="true">☁️ </span>
        <span className="tree-node-label">{name}</span>
        <small className="tree-node-count">({branches.length + tags.length})</small>
      </div>
      {isExpanded && (
        <div className="tree-node-children remote-reference-children">
          {branchTree.map((node) => (
            <BranchTreeNodeView
              key={node.id}
              node={node}
              isRemote={true}
              selection={branchSelection}
              selectionPrefix="remote:"
              pathPrefix={name}
              onSelectSelectionKey={onBranchSelection}
              onSelect={onSelectBranch}
              onCheckout={onCheckout}
            />
          ))}
          {tags.map((tag) => {
            const selectionKey = `remote:${tag.remote}/${tag.name}`;
            const selectionIndex = tagSelectionItems.indexOf(selectionKey);
            return (
              <div
                key={selectionKey}
                className={`tag-item-row tree-leaf-row ${tagSelection.selected.has(selectionKey) ? "selected" : ""}`}
                role="button"
                tabIndex={
                  tagSelection.focused === selectionKey ||
                  (!tagSelection.focused && selectionIndex === 0)
                    ? 0
                    : -1
                }
                aria-label={t("Tag {name}", { name: tag.name })}
                aria-pressed={tagSelection.selected.has(selectionKey)}
                data-selection-scope="tags"
                data-selection-index={selectionIndex}
                onMouseDown={(event) => tagSelection.onMouseDown(selectionKey, event)}
                onMouseEnter={(event) => tagSelection.onMouseEnter(selectionKey, event)}
                onClick={(event) => {
                  event.stopPropagation();
                  tagSelection.onClick(selectionKey, event);
                  onSelectTag(tag);
                }}
                onKeyDown={(event) => {
                  event.stopPropagation();
                  tagSelection.onKeyDown(
                    selectionKey,
                    event,
                    onTagSelection,
                    (index) => focusSelectionIndex(event.currentTarget, index),
                  );
                }}
              >
                <span className="branch-icon" aria-hidden="true">🏷️ </span>
                <span className="branch-name" title={tag.name}>{tag.name}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function SidebarGroup({
  label,
  count,
  children,
  initialLimit = 5,
  defaultExpanded = true,
  onClearSelection,
  onContextMenu,
}: {
  label: string;
  count?: number;
  children?: ReactNode;
  initialLimit?: number;
  defaultExpanded?: boolean;
  onClearSelection?: () => void;
  onContextMenu?: (event: ReactMouseEvent<HTMLDivElement>) => void;
}) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [showAll, setShowAll] = useState(false);

  const childArray = Children.toArray(children);
  const itemCount = count ?? childArray.length;
  const visibleChildren = showAll ? childArray : childArray.slice(0, initialLimit);

  return (
    <div
      className={`sidebar-read-group ${isExpanded ? "expanded" : "collapsed"}`}
      onMouseDown={(event) => {
        if (!onClearSelection) return;
        const target = event.target as HTMLElement;
        if (
          target.closest(
            ".tree-leaf-row, .tree-node-header, .sidebar-group, button, input, label",
          )
        ) {
          return;
        }
        onClearSelection();
      }}
    >
      <div
        className="sidebar-group"
        role="button"
        tabIndex={0}
        aria-expanded={isExpanded}
        onContextMenu={onContextMenu}
        onClick={() => setIsExpanded((prev) => !prev)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsExpanded((prev) => !prev);
          }
        }}
      >
        <span className={`group-chevron ${isExpanded ? "open" : ""}`} aria-hidden="true">
          ›
        </span>
        {label}
        {itemCount !== undefined && <small>({itemCount})</small>}
      </div>
      {isExpanded && childArray.length > 0 && (
        <div className="sidebar-items">
          {visibleChildren}
          {childArray.length > initialLimit && (
            <button
              type="button"
              className="show-more-btn"
              onClick={() => setShowAll((prev) => !prev)}
            >
              {showAll
                ? t("Show less")
                : t("Show all ({count})", { count: childArray.length })}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function StashControls({
  stashes,
  onApply,
  onContextMenu,
}: {
  stashes: RepositorySidebarDto["stashes"];
  onApply: (stash: StashItem) => void;
  onContextMenu: (stash: StashItem, x: number, y: number) => void;
}) {
  const [isExpanded, setIsExpanded] = useState(true);
  const [showAll, setShowAll] = useState(false);

  const visibleStashes = showAll ? stashes : stashes.slice(0, 5);

  return (
    <div className={`sidebar-read-group stash-controls ${isExpanded ? "expanded" : "collapsed"}`}>
      <div
        className="sidebar-group"
        role="button"
        tabIndex={0}
        aria-expanded={isExpanded}
        onClick={() => setIsExpanded((prev) => !prev)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsExpanded((prev) => !prev);
          }
        }}
      >
        <span className={`group-chevron ${isExpanded ? "open" : ""}`} aria-hidden="true">
          ›
        </span>
        {t("Stashes")}
        <small>({stashes.length})</small>
      </div>
      {isExpanded && (
        <div className="sidebar-items">
            {visibleStashes.map((stash) => (
              <div
                className="stash-item tree-leaf-row branch-item-row"
                key={stash.reference}
                role="button"
                tabIndex={0}
                aria-label={t("Stash {reference}: {message}", {
                  reference: stash.reference,
                  message: stash.message,
                })}
                title={stash.message}
                onDoubleClick={() => onApply(stash)}
                onContextMenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  onContextMenu(stash, event.clientX, event.clientY);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onApply(stash);
                  } else if (
                    event.key === "ContextMenu" ||
                    (event.shiftKey && event.key === "F10")
                  ) {
                    event.preventDefault();
                    const bounds = event.currentTarget.getBoundingClientRect();
                    onContextMenu(stash, bounds.left + 12, bounds.top + 12);
                  }
                }}
              >
                <span className="branch-icon" aria-hidden="true">◒ </span>
                <span className="branch-name">{stash.message}</span>
                <small className="stash-reference">{stash.reference}</small>
              </div>
            ))}
            {stashes.length > 5 && (
              <button
                type="button"
                className="show-more-btn"
                onClick={() => setShowAll((prev) => !prev)}
              >
                {showAll
                  ? t("Show less")
                  : t("Show all ({count})", { count: stashes.length })}
              </button>
            )}
        </div>
      )}
    </div>
  );
}

function OperationsView({ onError }: { onError: (error: unknown) => void }) {
  const [operations, setOperations] = useState<OperationRecordDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [copyState, setCopyState] = useState("");

  function refresh() {
    setLoading(true);
    getOperationHistory()
      .then(setOperations)
      .catch(onError)
      .finally(() => setLoading(false));
  }

  useEffect(refresh, [onError]);

  async function copyDiagnostics() {
    try {
      const diagnostics = await getDiagnostics();
      await navigator.clipboard.writeText(diagnostics);
      setCopyState(t("Diagnostics copied"));
    } catch (reason: unknown) {
      onError(reason);
    }
  }

  return (
    <section className="operations-view" aria-labelledby="operations-title">
      <div className="operations-heading">
        <div>
          <span className="eyebrow">{t("Recovery and diagnostics")}</span>
          <h1 id="operations-title">{t("Operation center")}</h1>
        </div>
        <div>
          <button type="button" onClick={refresh}>{t("Refresh")}</button>
          <button type="button" onClick={() => void copyDiagnostics()}>{t("Copy diagnostics")}</button>
        </div>
      </div>
      {copyState && <p role="status">{copyState}</p>}
      {loading ? (
        <div className="history-state" role="status">{t("Loading operations…")}</div>
      ) : operations.length === 0 ? (
        <div className="history-state">{t("No operations have been recorded.")}</div>
      ) : (
        <ol className="operation-list">
          {operations.map((operation) => (
            <li key={operation.id}>
              <span className={`operation-state ${operation.state}`}>{operationTerm(operation.state)}</span>
              <div>
                <strong>{operationTerm(operation.kind)}</strong>
                <span>{operation.summary}</span>
                {operation.diagnostic && <code>{operation.diagnostic}</code>}
              </div>
              <time>{operation.startedAt}</time>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function ErrorBanner({ title, message, detail, actionLabel, onAction }: { title: string; message: string; detail?: string; actionLabel?: string; onAction?: () => void }) {
  return (
    <div className="error-banner" role="alert">
      <div>
        <strong>{title}</strong>
        <span>{message}</span>
        {detail && <small>{detail}</small>}
      </div>
      {actionLabel && (
        <button
          className="control-button control-button--danger"
          type="button"
          onClick={onAction}
        >
          {actionLabel}
        </button>
      )}
    </div>
  );
}

function UnavailableRepository({ tab, onLocate }: { tab: SessionTabDto; onLocate: () => void }) {
  const name = repositoryName(tab.worktreePath);
  return (
    <div className="welcome-panel">
      <p className="eyebrow">{t("Repository unavailable")}</p>
      <h1>{t("{name} moved or was deleted.", { name })}</h1>
      <p>{tab.worktreePath}</p>
      <button
        className="control-button control-button--primary"
        type="button"
        onClick={onLocate}
      >
        {t("Locate repository")}
      </button>
    </div>
  );
}

const commitPanelMinHeight = 240;
const commitPanelDefaultHeight = 300;
const commitPanelMaxHeight = 480;
const commitPanelMinDiffHeight = 120;

function ChangesView({
  snapshot,
  refreshing,
  selectedPath,
  selectedTarget,
  panelWidth,
  onPanelWidth,
  onSelect,
  onSnapshot,
  onError,
}: {
  snapshot: RepositorySnapshotDto;
  refreshing: boolean;
  selectedPath?: string;
  selectedTarget: DiffTarget;
  panelWidth: number;
  onPanelWidth: (width: number) => void;
  onSelect: (path: string, target: DiffTarget) => void;
  onSnapshot: (snapshot: RepositorySnapshotDto) => void;
  onError: (error: unknown) => void;
}) {
  const unstaged = useMemo(
    () =>
      snapshot.changes.filter(
        (change) => change.worktreeStatus !== "." || change.conflict,
      ),
    [snapshot],
  );
  const staged = useMemo(
    () =>
      snapshot.changes.filter(
        (change) => change.indexStatus !== "." && change.indexStatus !== "?",
      ),
    [snapshot],
  );
  const unstagedPaths = useMemo(() => unstaged.map((change) => change.path), [unstaged]);
  const stagedPathsList = useMemo(() => staged.map((change) => change.path), [staged]);
  const unstagedSelection = useMultiSelection(unstagedPaths, "changes-unstaged");
  const stagedSelection = useMultiSelection(stagedPathsList, "changes-staged");
  const selected = snapshot.changes.find((change) => change.path === selectedPath);
  const [diff, setDiff] = useState<DiffDto>();
  const [diffLoading, setDiffLoading] = useState(false);
  const [showFileSearch, setShowFileSearch] = useState(false);
  const [fileFilter, setFileFilter] = useState("");
  const [showDiffSearch, setShowDiffSearch] = useState(false);
  const [diffSearchQuery, setDiffSearchQuery] = useState("");
  const [diffSearchMatchIndex, setDiffSearchMatchIndex] = useState(0);
  const [selectedLines, setSelectedLines] = useState<Set<string>>(new Set());
  const [operation, setOperation] = useState<string>();
  const mutationBlocked = Boolean(operation) || refreshing;
  const [summary, setSummary] = useState("");
  const [description, setDescription] = useState("");
  const [amend, setAmend] = useState(false);
  const [changeContextMenu, setChangeContextMenu] = useState<{
    x: number;
    y: number;
    target: DiffTarget;
  }>();
  const [stashCreateDialog, setStashCreateDialog] = useState<{
    paths: number[][];
    count: number;
  }>();
  const [commitExpanded, setCommitExpanded] = useState(true);
  const [commitPanelHeight, setCommitPanelHeight] = useState(() => {
    try {
      const saved = Number(localStorage.getItem("gitacorn:commit-panel-height"));
      if (
        Number.isFinite(saved) &&
        saved >= commitPanelMinHeight &&
        saved <= commitPanelMaxHeight
      ) {
        return saved;
      }
    } catch {
      // ignore
    }
    return commitPanelDefaultHeight;
  });
  const commitPanelHeightRef = useRef(commitPanelHeight);
  const summaryRef = useRef<HTMLTextAreaElement>(null);
  const descriptionRef = useRef<HTMLTextAreaElement>(null);
  const fileSearchInputRef = useRef<HTMLInputElement>(null);
  const diffSearchInputRef = useRef<HTMLInputElement>(null);
  const [livePanelWidth, setLivePanelWidth] = useState(panelWidth);
  const livePanelWidthRef = useRef(panelWidth);
  const draggedUnstagedPaths = useRef<string[]>([]);
  const pointerDrag = useRef<
    {
      path: string;
      source: DiffTarget;
      startX: number;
      startY: number;
      originalSelection: Set<string>;
    } | undefined
  >(undefined);
  const nativeDropHandled = useRef(false);
  const [activeDropTarget, setActiveDropTarget] = useState<DiffTarget>();
  const normalizedFileFilter = fileFilter.trim().toLocaleLowerCase();
  const filteredUnstaged = useMemo(
    () =>
      unstaged.filter((change) =>
        change.path.toLocaleLowerCase().includes(normalizedFileFilter),
      ),
    [normalizedFileFilter, unstaged],
  );
  const filteredStaged = useMemo(
    () =>
      staged.filter((change) =>
        change.path.toLocaleLowerCase().includes(normalizedFileFilter),
      ),
    [normalizedFileFilter, staged],
  );
  const normalizedDiffSearch = diffSearchQuery.toLocaleLowerCase();
  const diffSearchMatches = useMemo(
    () =>
      normalizedDiffSearch
        ? (diff?.hunks.flatMap((hunk) =>
            hunk.lines
              .filter((line) =>
                line.content.toLocaleLowerCase().includes(normalizedDiffSearch),
              )
              .map((line) => `${hunk.index}:${line.index}`),
          ) ?? [])
        : [],
    [diff, normalizedDiffSearch],
  );
  const activeDiffSearchMatch =
    diffSearchMatches.length > 0
      ? diffSearchMatches[
          Math.min(diffSearchMatchIndex, diffSearchMatches.length - 1)
        ]
      : undefined;

  useEffect(() => {
    setLivePanelWidth(panelWidth);
    livePanelWidthRef.current = panelWidth;
  }, [panelWidth]);

  useEffect(() => {
    const openFileSearch = () => setShowFileSearch(true);
    const openDiffSearch = () => setShowDiffSearch(true);
    window.addEventListener(openChangesFileSearchEvent, openFileSearch);
    window.addEventListener(openChangesDiffSearchEvent, openDiffSearch);
    return () => {
      window.removeEventListener(openChangesFileSearchEvent, openFileSearch);
      window.removeEventListener(openChangesDiffSearchEvent, openDiffSearch);
    };
  }, []);

  useEffect(() => {
    if (showFileSearch) fileSearchInputRef.current?.focus();
  }, [showFileSearch]);

  useEffect(() => {
    if (showDiffSearch) diffSearchInputRef.current?.focus();
  }, [showDiffSearch]);

  useEffect(() => {
    setDiffSearchMatchIndex(0);
  }, [diffSearchQuery, selectedPath, selectedTarget]);

  useEffect(() => {
    if (!changeContextMenu) return;
    const close = () => setChangeContextMenu(undefined);
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    window.addEventListener("click", close);
    window.addEventListener("blur", close);
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [changeContextMenu]);

  useEffect(() => {
    if (!selectedPath) return;
    const selection =
      selectedTarget === "staged" ? stagedSelection : unstagedSelection;
    if (selection.items.includes(selectedPath)) {
      selection.setSelected((current) =>
        current.size === 0 ? new Set([selectedPath]) : current,
      );
    }
  }, [selectedPath, selectedTarget, stagedSelection.items, unstagedSelection.items]);

  useEffect(() => {
    const dropTargetAt = (clientX: number, clientY: number) => {
      if (typeof document.elementFromPoint !== "function") return undefined;
      const target = document
        .elementFromPoint(clientX, clientY)
        ?.closest<HTMLElement>("[data-change-drop-target]")
        ?.dataset.changeDropTarget;
      return target === "staged" || target === "unstaged" ? target : undefined;
    };

    const handlePointerMove = (event: MouseEvent) => {
      const pending = pointerDrag.current;
      if (!pending) return;
      const moved =
        Math.abs(event.clientX - pending.startX) +
          Math.abs(event.clientY - pending.startY) >
        6;
      const target = moved ? dropTargetAt(event.clientX, event.clientY) : undefined;
      const oppositeTarget =
        target && target !== pending.source ? target : undefined;
      if (oppositeTarget) {
        const sourceSelection =
          pending.source === "staged" ? stagedSelection : unstagedSelection;
        sourceSelection.setSelected(new Set(pending.originalSelection));
      }
      setActiveDropTarget(oppositeTarget);
    };

    const handlePointerUp = (event: MouseEvent) => {
      const pending = pointerDrag.current;
      pointerDrag.current = undefined;
      setActiveDropTarget(undefined);
      if (!pending || nativeDropHandled.current) {
        nativeDropHandled.current = false;
        return;
      }
      const moved =
        Math.abs(event.clientX - pending.startX) +
          Math.abs(event.clientY - pending.startY) >
        6;
      const target = moved ? dropTargetAt(event.clientX, event.clientY) : undefined;
      if (!target || target === pending.source) return;
      const paths = pending.originalSelection.has(pending.path)
        ? [...pending.originalSelection]
        : [pending.path];
      moveSelectedPaths(pending.source, paths);
    };

    window.addEventListener("mousemove", handlePointerMove);
    window.addEventListener("mouseup", handlePointerUp);
    return () => {
      window.removeEventListener("mousemove", handlePointerMove);
      window.removeEventListener("mouseup", handlePointerUp);
    };
  }, [
    stagedSelection.selected,
    unstagedSelection.selected,
    staged,
    unstaged,
    mutationBlocked,
    snapshot.revision,
  ]);

  useEffect(() => {
    let active = true;
    setSelectedLines(new Set());
    if (!selected || selected.conflict) {
      setDiff(undefined);
      return () => {
        active = false;
      };
    }
    setDiffLoading(true);
    getDiff(
      snapshot.repository.id,
      selected.pathBytes,
      selectedTarget,
    )
      .then((value) => active && setDiff(value))
      .catch((reason: unknown) => active && onError(reason))
      .finally(() => active && setDiffLoading(false));
    return () => {
      active = false;
    };
  }, [
    onError,
    selected,
    selectedTarget,
    snapshot.repository.id,
    snapshot.revision,
  ]);

  async function mutate(
    label: string,
    action: () => Promise<RepositorySnapshotDto>,
  ): Promise<boolean> {
    if (refreshing) return false;
    try {
      setOperation(label);
      const next = await action();
      setSelectedLines(new Set());
      onSnapshot(next);
      return true;
    } catch (reason: unknown) {
      const error = normalizeAppError(reason);
      if (error.code === "staleRevision") {
        try {
          const latest = await getRepositorySnapshot(snapshot.repository.id);
          setSelectedLines(new Set());
          onSnapshot(latest);
        } catch (refreshReason: unknown) {
          onError(refreshReason);
        }
      } else {
        onError(reason);
      }
      return false;
    } finally {
      setOperation(undefined);
    }
  }

  function applyLines() {
    if (!selected || selectedLines.size === 0) return;
    const byHunk = new Map<number, number[]>();
    for (const key of selectedLines) {
      const [hunk, line] = key.split(":").map(Number);
      byHunk.set(hunk, [...(byHunk.get(hunk) ?? []), line]);
    }
    void mutate(selectedTarget === "staged" ? t("Unstaging lines…") : t("Staging lines…"), () =>
      applyPatchSelection(
        snapshot.repository.id,
        snapshot.revision,
        selected.pathBytes,
        selectedTarget,
        [...byHunk].map(([hunkIndex, lineIndices]) => ({
          hunkIndex,
          lineIndices,
        })),
      ),
    );
  }

  function discardSelected() {
    if (!selected || selectedTarget !== "unstaged") return;
    if (
      !confirmRepositoryMutation(
        t("Discard the displayed working-tree changes in {path}? This cannot be undone by GitAcorn.", { path: selected.path }),
      )
    ) {
      return;
    }
    void mutate(t("Discarding…"), () =>
      discardPath(
        snapshot.repository.id,
        snapshot.revision,
        selected.pathBytes,
        selected.worktreeStatus === "?",
      ),
    );
  }

  function submitCommit() {
    if (!summary.trim()) return;
    void mutate(amend ? t("Amending…") : t("Committing…"), () =>
      createCommit(snapshot.repository.id, snapshot.revision, {
        summary,
        description,
        amend,
      }),
    );
  }

  function selectionFor(target: DiffTarget) {
    return target === "staged" ? stagedSelection : unstagedSelection;
  }

  function changesFor(target: DiffTarget) {
    return target === "staged" ? staged : unstaged;
  }

  function clearOppositeSelection(target: DiffTarget) {
    if (target === "staged") unstagedSelection.clear();
    else stagedSelection.clear();
  }

  function selectedChangesFor(target: DiffTarget) {
    const selection = selectionFor(target);
    return changesFor(target).filter((change) =>
      selection.selected.has(change.path),
    );
  }

  function openChangeContextMenu(
    target: DiffTarget,
    path: string,
    x: number,
    y: number,
  ) {
    const selection = selectionFor(target);
    clearOppositeSelection(target);
    if (!selection.selected.has(path)) {
      selection.setSelected(new Set([path]));
      onSelect(path, target);
    }
    setChangeContextMenu({ target, x, y });
  }

  function openStashDialog(target: DiffTarget) {
    const changes = selectedChangesFor(target);
    if (changes.length === 0) return;
    setChangeContextMenu(undefined);
    setStashCreateDialog({
      paths: changes.map((change) => change.pathBytes),
      count: changes.length,
    });
  }

  function submitStash(name: string) {
    if (!stashCreateDialog) return;
    void mutate(t("Stashing selected files…"), () =>
      createStash(
        snapshot.repository.id,
        snapshot.revision,
        name,
        stashCreateDialog.paths,
      ),
    ).then((succeeded) => {
      if (!succeeded) return;
      setStashCreateDialog(undefined);
      unstagedSelection.clear();
      stagedSelection.clear();
    });
  }

  function discardContextSelection() {
    const changes = selectedChangesFor("unstaged");
    setChangeContextMenu(undefined);
    if (
      changes.length === 0 ||
      !confirmRepositoryMutation(
        t("Discard {count} selected files? This cannot be undone by GitAcorn.", {
          count: changes.length,
        }),
      )
    ) {
      return;
    }
    void mutate(t("Discarding…"), async () => {
      let next: RepositorySnapshotDto | undefined;
      let revision = snapshot.revision;
      for (const change of changes) {
        next = await discardPath(
          snapshot.repository.id,
          revision,
          change.pathBytes,
          change.worktreeStatus === "?",
        );
        revision = next.revision;
      }
      return next ?? snapshot;
    });
  }

  function beginUnstagedDrag(path: string, event: ReactDragEvent<HTMLElement>) {
    const paths = unstagedSelection.selected.has(path)
      ? [...unstagedSelection.selected]
      : [path];
    draggedUnstagedPaths.current = paths;
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("application/x-gitacorn-unstaged", paths.join("\n"));
    event.dataTransfer.setData("text/plain", paths.join("\n"));
  }

  function beginPointerDrag(
    path: string,
    source: DiffTarget,
    event: ReactMouseEvent<HTMLElement>,
  ) {
    if (event.button !== 0) return;
    nativeDropHandled.current = false;
    pointerDrag.current = {
      path,
      source,
      startX: event.clientX,
      startY: event.clientY,
      originalSelection: new Set(
        source === "staged"
          ? stagedSelection.selected
          : unstagedSelection.selected,
      ),
    };
  }

  function moveSelectedPaths(source: DiffTarget, paths: string[]) {
    if (mutationBlocked) return;
    const pathSet = new Set(paths);
    const changes = source === "staged" ? staged : unstaged;
    const pathBytes = changes
      .filter((change) => pathSet.has(change.path))
      .map((change) => change.pathBytes);
    if (pathBytes.length === 0) return;
    void mutate(
      source === "staged" ? t("Unstaging files…") : t("Staging files…"),
      () =>
        source === "staged"
          ? unstagePaths(snapshot.repository.id, snapshot.revision, pathBytes)
          : stagePaths(snapshot.repository.id, snapshot.revision, pathBytes),
    );
  }

  function dropOnStaged(event: ReactDragEvent<HTMLElement>) {
    event.preventDefault();
    nativeDropHandled.current = true;
    setActiveDropTarget(undefined);
    const paths = draggedUnstagedPaths.current;
    draggedUnstagedPaths.current = [];
    moveSelectedPaths("unstaged", paths);
  }

  const [stageSplitRatio, setStageSplitRatio] = useState(() => {
    try {
      const saved = localStorage.getItem("gitacorn:stage-split-ratio");
      if (saved) {
        const parsed = parseFloat(saved);
        if (!isNaN(parsed) && parsed >= 0.1 && parsed <= 0.9) {
          return parsed;
        }
      }
    } catch {
      // ignore
    }
    return 0.5;
  });

  const handleStageResizerMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    const filePanelEl = e.currentTarget.closest(".file-panel") as HTMLElement | null;
    if (!filePanelEl) return;
    const rect = filePanelEl.getBoundingClientRect();
    const resizerHeight = 7;
    const availableHeight = rect.height - resizerHeight;
    if (availableHeight <= 0) return;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const currentY = moveEvent.clientY - rect.top;
      const ratio = Math.max(0.1, Math.min(0.9, currentY / availableHeight));
      setStageSplitRatio(ratio);
      try {
        localStorage.setItem("gitacorn:stage-split-ratio", String(ratio));
      } catch {
        // ignore
      }
    };

    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    document.body.style.cursor = "row-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  const handleStageResizerKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setStageSplitRatio((prev) => {
        const next = Math.max(0.1, prev - 0.05);
        try {
          localStorage.setItem("gitacorn:stage-split-ratio", String(next));
        } catch {}
        return next;
      });
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setStageSplitRatio((prev) => {
        const next = Math.min(0.9, prev + 0.05);
        try {
          localStorage.setItem("gitacorn:stage-split-ratio", String(next));
        } catch {}
        return next;
      });
    }
  };

  const handleFilePanelResizerMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = livePanelWidthRef.current;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const deltaX = moveEvent.clientX - startX;
      const nextWidth = Math.max(160, Math.min(600, startWidth + deltaX));
      livePanelWidthRef.current = nextWidth;
      setLivePanelWidth(nextWidth);
    };

    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      onPanelWidth(livePanelWidthRef.current);
    };

    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  const handleFilePanelResizerKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      onPanelWidth(Math.max(160, livePanelWidthRef.current - 10));
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      onPanelWidth(Math.min(600, livePanelWidthRef.current + 10));
    }
  };

  const commitPanelMaximumFor = (target: Element) => {
    const diffPanel = target.closest(".diff-panel");
    const availableHeight = diffPanel?.getBoundingClientRect().height ?? 0;
    if (availableHeight <= 0) return commitPanelMaxHeight;
    return Math.max(
      commitPanelMinHeight,
      Math.min(
        commitPanelMaxHeight,
        availableHeight - commitPanelMinDiffHeight,
      ),
    );
  };

  const updateCommitPanelHeight = (height: number, maximum: number) => {
    const nextHeight = Math.max(
      commitPanelMinHeight,
      Math.min(maximum, height),
    );
    commitPanelHeightRef.current = nextHeight;
    setCommitPanelHeight(nextHeight);
    try {
      localStorage.setItem(
        "gitacorn:commit-panel-height",
        String(nextHeight),
      );
    } catch {
      // ignore
    }
  };

  const handleCommitPanelResizerMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    const startY = e.clientY;
    const startHeight = commitPanelHeightRef.current;
    const maximum = commitPanelMaximumFor(e.currentTarget);

    const onMouseMove = (moveEvent: MouseEvent) => {
      updateCommitPanelHeight(
        startHeight + startY - moveEvent.clientY,
        maximum,
      );
    };

    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    document.body.style.cursor = "row-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  const handleCommitPanelResizerKeyDown = (e: React.KeyboardEvent) => {
    const maximum = commitPanelMaximumFor(e.currentTarget);
    if (e.key === "ArrowUp") {
      e.preventDefault();
      updateCommitPanelHeight(commitPanelHeightRef.current + 10, maximum);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      updateCommitPanelHeight(commitPanelHeightRef.current - 10, maximum);
    } else if (e.key === "Home") {
      e.preventDefault();
      updateCommitPanelHeight(commitPanelMinHeight, maximum);
    } else if (e.key === "End") {
      e.preventDefault();
      updateCommitPanelHeight(maximum, maximum);
    }
  };

  const closeFileSearch = () => {
    setShowFileSearch(false);
    setFileFilter("");
  };

  const closeDiffSearch = () => {
    setShowDiffSearch(false);
    setDiffSearchQuery("");
    setDiffSearchMatchIndex(0);
  };

  const moveDiffSearchMatch = (direction: -1 | 1) => {
    if (diffSearchMatches.length === 0) return;
    setDiffSearchMatchIndex(
      (current) =>
        (current + direction + diffSearchMatches.length) %
        diffSearchMatches.length,
    );
  };

  const handleFileSearchEscape = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape" && showFileSearch) {
      event.preventDefault();
      event.stopPropagation();
      closeFileSearch();
    }
  };

  const handleDiffSearchEscape = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape" && showDiffSearch) {
      event.preventDefault();
      event.stopPropagation();
      closeDiffSearch();
    }
  };

  return (
    <div
      className="changes-layout"
      style={{ "--file-panel-width": `${livePanelWidth}px` } as CSSProperties}
    >
      <section
        className="file-panel"
        aria-label={t("Changed files")}
        onKeyDownCapture={handleFileSearchEscape}
      >
        {showFileSearch && (
          <div className="panel-searchbar file-searchbar" role="search">
            <input
              ref={fileSearchInputRef}
              className="control-input"
              type="search"
              aria-label={t("Filter changed files")}
              placeholder={t("Filter by file name or path")}
              value={fileFilter}
              onChange={(event) => setFileFilter(event.currentTarget.value)}
            />
            <span className="search-result-count">
              {filteredUnstaged.length + filteredStaged.length}
            </span>
            <button
              className="search-close-button"
              type="button"
              aria-label={t("Close file filter")}
              onClick={closeFileSearch}
            >
              ×
            </button>
          </div>
        )}
        <div style={{ flex: `${stageSplitRatio} 1 0%`, display: "flex", flexDirection: "column", minHeight: 0 }}>
          <ChangeSection
            title={t("Unstaged")}
            target="unstaged"
            changes={filteredUnstaged}
            selectedPath={selectedPath}
            selectedTarget={selectedTarget}
            selection={unstagedSelection}
            onSelect={onSelect}
            onActivateSelection={() => clearOppositeSelection("unstaged")}
            onContextMenu={(path, x, y) =>
              openChangeContextMenu("unstaged", path, x, y)
            }
            onDragStart={beginUnstagedDrag}
            onPointerDragStart={(path, event) =>
              beginPointerDrag(path, "unstaged", event)
            }
            onDragEnd={() => {
              draggedUnstagedPaths.current = [];
              pointerDrag.current = undefined;
              setActiveDropTarget(undefined);
            }}
            dropActive={activeDropTarget === "unstaged"}
            isChangeDropTarget="unstaged"
          />
        </div>
        <div
          className="stage-resizer"
          role="separator"
          aria-orientation="horizontal"
          aria-label={t("Stage and Unstage split height")}
          tabIndex={0}
          onMouseDown={handleStageResizerMouseDown}
          onKeyDown={handleStageResizerKeyDown}
        />
        <div style={{ flex: `${1 - stageSplitRatio} 1 0%`, display: "flex", flexDirection: "column", minHeight: 0 }}>
          <ChangeSection
            title={t("Staged")}
            target="staged"
            changes={filteredStaged}
            selectedPath={selectedPath}
            selectedTarget={selectedTarget}
            selection={stagedSelection}
            onSelect={onSelect}
            onActivateSelection={() => clearOppositeSelection("staged")}
            onContextMenu={(path, x, y) =>
              openChangeContextMenu("staged", path, x, y)
            }
            onPointerDragStart={(path, event) =>
              beginPointerDrag(path, "staged", event)
            }
            dropActive={activeDropTarget === "staged"}
            onDragEnter={(event) => {
              event.preventDefault();
              if (draggedUnstagedPaths.current.length > 0) {
                setActiveDropTarget("staged");
              }
            }}
            onDragOver={(event) => {
              if (draggedUnstagedPaths.current.length === 0) return;
              event.preventDefault();
              event.dataTransfer.dropEffect = "move";
            }}
            onDragLeave={(event) => {
              if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
                setActiveDropTarget(undefined);
              }
            }}
            onDrop={dropOnStaged}
            isChangeDropTarget="staged"
          />
        </div>
        <div
          className="file-panel-resizer"
          role="separator"
          aria-orientation="vertical"
          aria-label={t("File panel width")}
          tabIndex={0}
          onMouseDown={handleFilePanelResizerMouseDown}
          onKeyDown={handleFilePanelResizerKeyDown}
        />
      </section>
      <section
        className="selected-file-panel diff-panel"
        onKeyDownCapture={handleDiffSearchEscape}
      >
        <div className="diff-content">
          {selected ? (
            <>
            <div className="diff-toolbar">
              <div>
                <span className="eyebrow">
                  {selectedTarget === "staged" ? t("Staged diff") : t("Unstaged diff")}
                </span>
                <strong>{selected.path}</strong>
              </div>
              {selected.conflict ? (
                <div className="conflict-actions" aria-label={t("Conflict resolution")}>
                  <button
                    type="button"
                    disabled={mutationBlocked}
                    onClick={() =>
                      void mutate(t("Using our version…"), () =>
                        resolveConflict(
                          snapshot.repository.id,
                          snapshot.revision,
                          selected.pathBytes,
                          "ours",
                        ),
                      )
                    }
                  >
                    {t("Use ours")}
                  </button>
                  <button
                    type="button"
                    disabled={mutationBlocked}
                    onClick={() =>
                      void mutate(t("Using their version…"), () =>
                        resolveConflict(
                          snapshot.repository.id,
                          snapshot.revision,
                          selected.pathBytes,
                          "theirs",
                        ),
                      )
                    }
                  >
                    {t("Use theirs")}
                  </button>
                  <button
                    type="button"
                    disabled={mutationBlocked}
                    onClick={() =>
                      void mutate(t("Marking resolved…"), () =>
                        resolveConflict(
                          snapshot.repository.id,
                          snapshot.revision,
                          selected.pathBytes,
                          "markResolved",
                        ),
                      )
                    }
                  >
                    {t("Mark current content resolved")}
                  </button>
                  {snapshot.operation !== "autostashConflict" && (
                    <button
                      type="button"
                      className="danger-button"
                      disabled={mutationBlocked}
                      onClick={() => {
                        if (
                          confirmRepositoryMutation(
                            snapshot.operation === "rebase"
                              ? t("Abort this rebase and restore the previous branch state?")
                              : t("Abort this merge and restore the pre-merge working tree?"),
                          )
                        ) {
                          void mutate(
                            snapshot.operation === "rebase"
                              ? t("Aborting rebase…")
                              : t("Aborting merge…"),
                            () =>
                              snapshot.operation === "rebase"
                                ? abortRebase(
                                    snapshot.repository.id,
                                    snapshot.revision,
                                  )
                                : abortMerge(
                                    snapshot.repository.id,
                                    snapshot.revision,
                                  ),
                          );
                        }
                      }}
                    >
                      {snapshot.operation === "rebase"
                        ? t("Abort rebase…")
                        : t("Abort merge…")}
                    </button>
                  )}
                </div>
              ) : (
              <div>
                <button
                  type="button"
                  disabled={selectedLines.size === 0 || mutationBlocked}
                  onClick={applyLines}
                >
                  {selectedTarget === "staged" ? t("Unstage selected lines") : t("Stage selected lines")}
                </button>
                {selectedTarget === "unstaged" && (
                  <button
                    className="danger-button"
                    type="button"
                    disabled={mutationBlocked}
                    onClick={discardSelected}
                  >
                    {t("Discard…")}
                  </button>
                )}
              </div>
              )}
            </div>
            {showDiffSearch && (
              <form
                className="panel-searchbar diff-searchbar"
                role="search"
                onSubmit={(event) => {
                  event.preventDefault();
                  moveDiffSearchMatch(1);
                }}
              >
                <input
                  ref={diffSearchInputRef}
                  className="control-input"
                  type="search"
                  aria-label={t("Search file changes")}
                  placeholder={t("Search changed content")}
                  value={diffSearchQuery}
                  onChange={(event) =>
                    setDiffSearchQuery(event.currentTarget.value)
                  }
                />
                <span className="search-result-count" role="status">
                  {diffSearchMatches.length === 0
                    ? "0 / 0"
                    : `${Math.min(diffSearchMatchIndex + 1, diffSearchMatches.length)} / ${diffSearchMatches.length}`}
                </span>
                <button
                  className="search-navigation-button"
                  type="button"
                  aria-label={t("Previous match")}
                  disabled={diffSearchMatches.length === 0}
                  onClick={() => moveDiffSearchMatch(-1)}
                >
                  ↑
                </button>
                <button
                  className="search-navigation-button"
                  type="submit"
                  aria-label={t("Next match")}
                  disabled={diffSearchMatches.length === 0}
                >
                  ↓
                </button>
                <button
                  className="search-close-button"
                  type="button"
                  aria-label={t("Close content search")}
                  onClick={closeDiffSearch}
                >
                  ×
                </button>
              </form>
            )}
            {operation && <div className="operation-status" role="status">{operation}</div>}
            {selected.conflict ? (
              <div className="conflict-panel" role="region" aria-label={t("Conflict resolution guidance")}>
                <h2>{t("Resolve merge conflict")}</h2>
                <p>
                  {t("Choose one side, or edit the file in your editor and mark the current content resolved. Aborting restores the state from before the merge.")}
                </p>
              </div>
            ) : diffLoading ? (
              <div className="diff-state" role="status">{t("Loading diff…")}</div>
            ) : diff?.binary ? (
              <div className="diff-state">{t("Binary file. Use the whole-file action.")}</div>
            ) : diff && diff.hunks.length > 0 ? (
              <DiffRenderer
                diff={diff}
                selectedLines={selectedLines}
                onSelectionChange={setSelectedLines}
                onToggleLine={(key) =>
                  setSelectedLines((current) => {
                    const next = new Set(current);
                    if (next.has(key)) next.delete(key);
                    else next.add(key);
                    return next;
                  })
                }
                onApplyHunk={(hunkIndex) =>
                  void mutate(
                    selectedTarget === "staged" ? t("Unstaging hunk…") : t("Staging hunk…"),
                    () =>
                      applyPatchSelection(
                        snapshot.repository.id,
                        snapshot.revision,
                        selected.pathBytes,
                        selectedTarget,
                        [{ hunkIndex, lineIndices: [] }],
                      ),
                  )
                }
                actionLabel={selectedTarget === "staged" ? t("Unstage hunk") : t("Stage hunk")}
                actionDisabled={mutationBlocked}
                searchQuery={diffSearchQuery}
                activeSearchKey={activeDiffSearchMatch}
              />
            ) : (
              <div className="diff-state">{t("No text diff is available for this side.")}</div>
            )}
            </>
          ) : (
            <div className="empty-selection">
              <span className="file-glyph" aria-hidden="true" />
              <h1>
                {snapshot.changes.length === 0
                  ? t("Working tree clean")
                  : t("Select a changed file")}
              </h1>
              <p>
                {snapshot.changes.length === 0
                  ? t("There are no staged or unstaged changes.")
                  : t("Choose a file to inspect and stage its diff.")}
              </p>
            </div>
          )}
        </div>
        <aside
          className="commit-panel"
          aria-label={t("Commit form")}
          style={commitExpanded ? { height: `${commitPanelHeight}px` } : undefined}
        >
          {commitExpanded && (
            <div
              className="commit-panel-resizer"
              role="separator"
              aria-orientation="horizontal"
              aria-label={t("Commit panel height")}
              aria-valuemin={commitPanelMinHeight}
              aria-valuemax={commitPanelMaxHeight}
              aria-valuenow={commitPanelHeight}
              tabIndex={0}
              onMouseDown={handleCommitPanelResizerMouseDown}
              onKeyDown={handleCommitPanelResizerKeyDown}
            />
          )}
          <button
            className="commit-panel-toggle"
            type="button"
            aria-label={t("Commit form")}
            aria-expanded={commitExpanded}
            onClick={() => setCommitExpanded((expanded) => !expanded)}
          >
            <span>
              <span
                className={`commit-panel-chevron ${commitExpanded ? "expanded" : ""}`}
                aria-hidden="true"
              >
                ›
              </span>
              <strong>{t("Commit")}</strong>
            </span>
            <span className="commit-staged-count">
              {staged.length} {t("Staged")}
            </span>
          </button>
          {commitExpanded && (
            <div className="commit-fields">
              <textarea
                className="commit-summary"
                ref={summaryRef}
                aria-label={t("Commit summary")}
                placeholder={t("Summary")}
                value={summary}
                onChange={(event) => setSummary(event.currentTarget.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    descriptionRef.current?.focus();
                  }
                }}
              />
              <textarea
                className="commit-description"
                ref={descriptionRef}
                aria-label={t("Commit description")}
                placeholder={t("Description (optional)")}
                value={description}
                onChange={(event) => setDescription(event.currentTarget.value)}
                onKeyDown={(event) => {
                  if (
                    event.key === "Backspace" &&
                    event.currentTarget.selectionStart === 0 &&
                    event.currentTarget.selectionEnd === 0
                  ) {
                    event.preventDefault();
                    const summaryInput = summaryRef.current;
                    if (!summaryInput) return;
                    summaryInput.focus();
                    const end = summaryInput.value.length;
                    summaryInput.setSelectionRange(end, end);
                  }
                }}
              />
              <label className="amend-control">
                <input
                  type="checkbox"
                  checked={amend}
                  onChange={(event) => setAmend(event.currentTarget.checked)}
                />
                {t("Amend previous commit")}
              </label>
              <button
                className="primary-action"
                type="button"
                disabled={!summary.trim() || (!amend && staged.length === 0) || Boolean(operation)}
                onClick={submitCommit}
              >
                {t("{action} to {branch}", {
                  action: amend ? t("Amend") : t("Commit"),
                  branch: snapshot.head.name ?? "HEAD",
                })}
              </button>
            </div>
          )}
        </aside>
      </section>
      {changeContextMenu && (
        <div
          className="remote-context-menu"
          role="menu"
          aria-label={t("File actions")}
          style={{ left: changeContextMenu.x, top: changeContextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            role="menuitem"
            disabled={mutationBlocked}
            onClick={() => {
              const target = changeContextMenu.target;
              const paths = [...selectionFor(target).selected];
              setChangeContextMenu(undefined);
              moveSelectedPaths(target, paths);
            }}
          >
            {changeContextMenu.target === "staged"
              ? t("Unstage file")
              : t("Stage file")}
          </button>
          {changeContextMenu.target === "unstaged" && (
            <button
              type="button"
              role="menuitem"
              className="danger-button"
              disabled={mutationBlocked}
              onClick={discardContextSelection}
            >
              {t("Discard…")}
            </button>
          )}
          <button
            type="button"
            role="menuitem"
            disabled={mutationBlocked}
            onClick={() => openStashDialog(changeContextMenu.target)}
          >
            {t("Stash…")}
          </button>
        </div>
      )}
      {stashCreateDialog && (
        <StashCreateDialog
          count={stashCreateDialog.count}
          busy={Boolean(operation)}
          onClose={() => {
            if (!operation) setStashCreateDialog(undefined);
          }}
          onCreate={submitStash}
        />
      )}
    </div>
  );
}

function ChangeSection({
  title,
  target,
  changes,
  selectedPath,
  selectedTarget,
  selection,
  onSelect,
  onActivateSelection,
  onContextMenu,
  onDragStart,
  onPointerDragStart,
  onDragEnd,
  dropActive = false,
  onDragEnter,
  onDragOver,
  onDragLeave,
  onDrop,
  isChangeDropTarget,
}: {
  title: string;
  target: DiffTarget;
  changes: FileChangeDto[];
  selectedPath?: string;
  selectedTarget?: DiffTarget;
  selection?: MultiSelection;
  onSelect: (path: string, target: DiffTarget) => void;
  onActivateSelection?: () => void;
  onContextMenu?: (path: string, x: number, y: number) => void;
  onDragStart?: (path: string, event: ReactDragEvent<HTMLElement>) => void;
  onPointerDragStart?: (path: string, event: ReactMouseEvent<HTMLElement>) => void;
  onDragEnd?: () => void;
  dropActive?: boolean;
  onDragEnter?: (event: ReactDragEvent<HTMLElement>) => void;
  onDragOver?: (event: ReactDragEvent<HTMLElement>) => void;
  onDragLeave?: (event: ReactDragEvent<HTMLElement>) => void;
  onDrop?: (event: ReactDragEvent<HTMLElement>) => void;
  isChangeDropTarget?: DiffTarget;
}) {
  return (
    <div
      className={`change-section ${dropActive ? "drop-target" : ""}`}
      onDragEnter={onDragEnter}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      data-change-drop-target={isChangeDropTarget}
    >
      <div className="panel-heading"><h2>{title}</h2><span>{changes.length}</span></div>
      {changes.length === 0 ? (
        <div className="panel-empty">
          {target === "staged" ? t("No staged changes.") : t("No unstaged changes.")}
        </div>
      ) : (
        <VirtualChangeList
          changes={changes}
          target={target}
          selectedPath={selectedPath}
          selectedTarget={selectedTarget}
          selection={selection}
          onSelect={onSelect}
          onActivateSelection={onActivateSelection}
          onContextMenu={onContextMenu}
          onDragStart={onDragStart}
          onPointerDragStart={onPointerDragStart}
          onDragEnd={onDragEnd}
        />
      )}
    </div>
  );
}

function VirtualChangeList({
  changes,
  target,
  selectedPath,
  selectedTarget,
  selection,
  onSelect,
  onActivateSelection,
  onContextMenu,
  onDragStart,
  onPointerDragStart,
  onDragEnd,
}: {
  changes: FileChangeDto[];
  target: DiffTarget;
  selectedPath?: string;
  selectedTarget?: DiffTarget;
  selection?: MultiSelection;
  onSelect: (path: string, target: DiffTarget) => void;
  onActivateSelection?: () => void;
  onContextMenu?: (path: string, x: number, y: number) => void;
  onDragStart?: (path: string, event: ReactDragEvent<HTMLElement>) => void;
  onPointerDragStart?: (path: string, event: ReactMouseEvent<HTMLElement>) => void;
  onDragEnd?: () => void;
}) {
  const rowHeight = 34;
  const [scrollTop, setScrollTop] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const blankDrag = useRef<
    | {
        startY: number;
        baseSelection: Set<string>;
        moved: boolean;
      }
    | undefined
  >(undefined);
  const [selectionBand, setSelectionBand] = useState<
    { top: number; height: number } | undefined
  >(undefined);
  const visibleCount = 20;
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - 4);
  const end = Math.min(changes.length, start + visibleCount + 8);

  useEffect(() => {
    const continueBlankDrag = (event: MouseEvent) => {
      const pending = blankDrag.current;
      const list = listRef.current;
      const space = list?.querySelector<HTMLElement>(".virtual-list-space");
      if (!pending || !space || (event.buttons & 1) === 0) return;

      const currentY = event.clientY - space.getBoundingClientRect().top;
      if (Math.abs(currentY - pending.startY) > 4) pending.moved = true;

      const totalHeight = changes.length * rowHeight;
      const top = Math.min(pending.startY, currentY);
      const bottom = Math.max(pending.startY, currentY);
      const clippedTop = Math.max(0, Math.min(totalHeight, top));
      const clippedBottom = Math.max(0, Math.min(totalHeight, bottom));
      setSelectionBand({
        top: clippedTop,
        height: Math.max(1, clippedBottom - clippedTop),
      });

      if (!pending.moved || bottom < 0 || top >= totalHeight) {
        selection?.setSelected(new Set(pending.baseSelection));
        return;
      }

      const firstIndex = Math.max(0, Math.floor(Math.max(0, top) / rowHeight));
      const lastIndex = Math.min(
        changes.length - 1,
        Math.floor(Math.max(0, Math.min(totalHeight - 1, bottom)) / rowHeight),
      );
      const range = changes
        .slice(firstIndex, lastIndex + 1)
        .map((change) => change.path);
      selection?.setSelected(new Set([...pending.baseSelection, ...range]));
    };

    const stopBlankDrag = () => {
      blankDrag.current = undefined;
      setSelectionBand(undefined);
    };

    window.addEventListener("mousemove", continueBlankDrag);
    window.addEventListener("mouseup", stopBlankDrag);
    return () => {
      window.removeEventListener("mousemove", continueBlankDrag);
      window.removeEventListener("mouseup", stopBlankDrag);
    };
  }, [changes, selection]);

  return (
    <div
      ref={listRef}
      className="change-list"
      onMouseDown={(event) => {
        if (
          event.button !== 0 ||
          (event.target as HTMLElement).closest(".change-row")
        ) {
          return;
        }
        event.preventDefault();
        onActivateSelection?.();
        const space = event.currentTarget.querySelector<HTMLElement>(
          ".virtual-list-space",
        );
        if (!space) return;
        const additive = event.ctrlKey || event.metaKey;
        const baseSelection = new Set(
          additive ? selection?.selected ?? [] : [],
        );
        if (!additive) selection?.clear();
        blankDrag.current = {
          startY: event.clientY - space.getBoundingClientRect().top,
          baseSelection,
          moved: false,
        };
        setSelectionBand(undefined);
      }}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <div className="virtual-list-space" style={{ height: changes.length * rowHeight }}>
        <div
          className="virtual-list-items"
          style={{ transform: `translateY(${start * rowHeight}px)` }}
        >
          {changes.slice(start, end).map((change) => {
            const partial =
              change.indexStatus !== "." &&
              change.indexStatus !== "?" &&
              (change.worktreeStatus !== "." || change.conflict);
            return (
              <button
                className={
                  (selection
                    ? selection.selected.has(change.path)
                    : selectedPath === change.path && selectedTarget === target)
                    ? "change-row selected"
                    : "change-row"
                }
                type="button"
                aria-pressed={selection?.selected.has(change.path) ?? false}
                tabIndex={
                  selection?.focused === change.path ||
                  (!selection?.focused && changes.indexOf(change) === 0)
                    ? 0
                    : -1
                }
                key={`${target}-${change.path}-${change.indexStatus}-${change.worktreeStatus}`}
                data-selection-scope={`changes-${target}`}
                data-selection-index={changes.indexOf(change)}
                onMouseDown={(event) => {
                  onActivateSelection?.();
                  selection?.onMouseDown(change.path, event);
                  onPointerDragStart?.(change.path, event);
                }}
                onMouseEnter={(event) => selection?.onMouseEnter(change.path, event)}
                onClick={(event) => {
                  onActivateSelection?.();
                  selection?.onClick(change.path, event);
                  onSelect(change.path, target);
                }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  onContextMenu?.(
                    change.path,
                    event.clientX,
                    event.clientY,
                  );
                }}
                onKeyDown={(event) => {
                  if (
                    event.key === "ContextMenu" ||
                    (event.shiftKey && event.key === "F10")
                  ) {
                    event.preventDefault();
                    const bounds = event.currentTarget.getBoundingClientRect();
                    onContextMenu?.(
                      change.path,
                      bounds.left + 12,
                      bounds.top + 12,
                    );
                    return;
                  }
                  onActivateSelection?.();
                  selection?.onKeyDown(
                    change.path,
                    event,
                    (item) => onSelect(item, target),
                    (index) => {
                      const list = event.currentTarget.closest<HTMLElement>(".change-list");
                      if (list) {
                        list.scrollTop = Math.max(0, index * rowHeight - rowHeight);
                      }
                      focusSelectionIndex(event.currentTarget, index);
                    },
                  );
                }}
                onDragStart={(event) => onDragStart?.(change.path, event)}
                onDragEnd={onDragEnd}
                title={change.path}
              >
                <span className={`status-badge ${change.conflict ? "conflict" : ""}`}>
                  {change.conflict
                    ? "!"
                    : target === "staged"
                      ? change.indexStatus
                      : change.worktreeStatus}
                </span>
                <span className="change-path">{change.path}</span>
                {partial && <small>{t("partial")}</small>}
                {!partial && change.submodule && <small>{t("submodule")}</small>}
              </button>
            );
          })}
        </div>
        {selectionBand ? (
          <div
            className="selection-band"
            style={{
              top: selectionBand.top,
              height: selectionBand.height,
            }}
          />
        ) : null}
      </div>
    </div>
  );
}

function DiffRenderer({
  diff,
  selectedLines,
  onSelectionChange,
  onToggleLine,
  onApplyHunk,
  actionLabel,
  actionDisabled,
  readOnly = false,
  searchQuery = "",
  activeSearchKey,
}: {
  diff: DiffDto;
  selectedLines: Set<string>;
  onSelectionChange: (selection: Set<string>) => void;
  onToggleLine: (key: string) => void;
  onApplyHunk: (hunkIndex: number) => void;
  actionLabel: string;
  actionDisabled: boolean;
  readOnly?: boolean;
  searchQuery?: string;
  activeSearchKey?: string;
}) {
  const selectableKeys = useMemo(
    () =>
      diff.hunks.flatMap((hunk) =>
        hunk.lines
          .filter((line) => line.selectable)
          .map((line) => `${hunk.index}:${line.index}`),
      ),
    [diff],
  );
  const selectableIndices = useMemo(
    () => new Map(selectableKeys.map((key, index) => [key, index])),
    [selectableKeys],
  );
  const dragSelection = useRef<
    | {
        anchorIndex: number;
        baseline: Set<string>;
        select: boolean;
      }
    | undefined
  >(undefined);
  const [dragSelecting, setDragSelecting] = useState(false);
  const diffScrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const finishDragSelection = () => {
      dragSelection.current = undefined;
      setDragSelecting(false);
    };
    window.addEventListener("mouseup", finishDragSelection);
    window.addEventListener("blur", finishDragSelection);
    return () => {
      window.removeEventListener("mouseup", finishDragSelection);
      window.removeEventListener("blur", finishDragSelection);
    };
  }, []);

  useEffect(() => {
    if (!activeSearchKey) return;
    const container = diffScrollRef.current;
    const line = container?.querySelector<HTMLElement>(
      `[data-diff-key="${activeSearchKey}"]`,
    );
    if (line && typeof line.scrollIntoView === "function") {
      line.scrollIntoView({ block: "center" });
      return;
    }
    const hunkIndex = activeSearchKey.split(":", 1)[0];
    const hunk = container?.querySelector<HTMLElement>(
      `[data-hunk-index="${hunkIndex}"]`,
    );
    if (hunk && typeof hunk.scrollIntoView === "function") {
      hunk.scrollIntoView({ block: "center" });
    }
  }, [activeSearchKey]);

  function selectDragRange(key: string) {
    const drag = dragSelection.current;
    const currentIndex = selectableIndices.get(key);
    if (!drag || currentIndex === undefined) return;
    const next = new Set(drag.baseline);
    const start = Math.min(drag.anchorIndex, currentIndex);
    const end = Math.max(drag.anchorIndex, currentIndex);
    for (const rangeKey of selectableKeys.slice(start, end + 1)) {
      if (drag.select) next.add(rangeKey);
      else next.delete(rangeKey);
    }
    onSelectionChange(next);
  }

  function beginDragSelection(
    key: string,
    event: ReactMouseEvent<HTMLButtonElement>,
  ) {
    if (event.button !== 0) return;
    const anchorIndex = selectableIndices.get(key);
    if (anchorIndex === undefined) return;
    event.preventDefault();
    dragSelection.current = {
      anchorIndex,
      baseline: new Set(selectedLines),
      select: !selectedLines.has(key),
    };
    setDragSelecting(true);
    selectDragRange(key);
  }

  function extendDragSelection(
    key: string,
    event: ReactMouseEvent<HTMLButtonElement>,
  ) {
    if (!dragSelection.current) return;
    if ((event.buttons & 1) === 0) {
      dragSelection.current = undefined;
      setDragSelecting(false);
      return;
    }
    selectDragRange(key);
  }

  return (
    <div
      ref={diffScrollRef}
      className={`diff-scroll ${dragSelecting ? "drag-selecting" : ""}`}
      aria-label={t("File diff")}
      title={readOnly ? undefined : t("Click or drag across changed lines to select them.")}
    >
      {diff.hunks.map((hunk) => (
        <div
          className="diff-hunk"
          key={hunk.index}
          data-hunk-index={hunk.index}
        >
          <div className="diff-hunk-header">
            <code>{hunk.header}</code>
            {!readOnly && (
              <button
                type="button"
                disabled={actionDisabled}
                onClick={() => onApplyHunk(hunk.index)}
              >
                {actionLabel}
              </button>
            )}
          </div>
          <VirtualDiffLines
            hunkIndex={hunk.index}
            lines={hunk.lines}
            selectedLines={selectedLines}
            onToggleLine={onToggleLine}
            onBeginSelection={beginDragSelection}
            onExtendSelection={extendDragSelection}
            readOnly={readOnly}
            searchQuery={searchQuery}
            activeSearchKey={activeSearchKey}
          />
        </div>
      ))}
    </div>
  );
}

const SYNTAX_KEYWORDS = new Set([
  "abstract", "as", "async", "await", "break", "case", "catch", "class", "const",
  "continue", "debugger", "default", "def", "delete", "do", "else", "enum",
  "export", "extends", "false", "final", "finally", "fn", "for", "from",
  "function", "if", "implements", "import", "in", "instanceof", "interface",
  "is", "let", "loop", "match", "mod", "move", "mut", "new", "null", "of",
  "package", "private", "protected", "pub", "public", "return", "self", "static",
  "struct", "super", "switch", "this", "throw", "trait", "true", "try", "type",
  "typeof", "undefined", "use", "var", "void", "while", "with", "yield"
]);

const SYNTAX_TYPES = new Set([
  "string", "number", "boolean", "any", "unknown", "never", "object", "symbol",
  "bigint", "void", "Promise", "Record", "Array", "Set", "Map", "Option", "Result",
  "Vec", "String", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "usize",
  "isize", "f32", "f64", "bool", "char", "str"
]);

export function highlightCodeLine(code: string): React.ReactNode[] {
  if (!code) return [];
  const nodes: React.ReactNode[] = [];
  const regex = /(\/\/.*$|#.*$|\/\*[\s\S]*?\*\/|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|\b0x[0-9a-fA-F]+\b|\b\d+(?:\.\d+)?\b|\b[a-zA-Z_$][a-zA-Z0-9_$]*\b|[<>/=+\-*%!&|^~?:]+|[{}()\[\],.;])/gm;

  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(code)) !== null) {
    if (match.index > lastIndex) {
      nodes.push(code.slice(lastIndex, match.index));
    }
    const token = match[0];
    const key = `${match.index}-${token}`;

    if (token.startsWith("//") || token.startsWith("#") || token.startsWith("/*")) {
      nodes.push(<span key={key} className="tok-comment">{token}</span>);
    } else if (token.startsWith('"') || token.startsWith("'") || token.startsWith("`")) {
      nodes.push(<span key={key} className="tok-string">{token}</span>);
    } else if (/^\d/.test(token)) {
      nodes.push(<span key={key} className="tok-number">{token}</span>);
    } else if (SYNTAX_KEYWORDS.has(token)) {
      nodes.push(<span key={key} className="tok-keyword">{token}</span>);
    } else if (SYNTAX_TYPES.has(token)) {
      nodes.push(<span key={key} className="tok-type">{token}</span>);
    } else if (/^[a-zA-Z_$]/.test(token)) {
      const rest = code.slice(regex.lastIndex).trimStart();
      if (rest.startsWith("(")) {
        nodes.push(<span key={key} className="tok-func">{token}</span>);
      } else {
        nodes.push(token);
      }
    } else if (/^[<>/=+\-*%!&|^~?:]+$/.test(token)) {
      nodes.push(<span key={key} className="tok-operator">{token}</span>);
    } else {
      nodes.push(<span key={key} className="tok-punctuation">{token}</span>);
    }

    lastIndex = regex.lastIndex;
  }

  if (lastIndex < code.length) {
    nodes.push(code.slice(lastIndex));
  }

  return nodes;
}

function VirtualDiffLines({
  hunkIndex,
  lines,
  selectedLines,
  onToggleLine,
  onBeginSelection,
  onExtendSelection,
  readOnly = false,
  searchQuery = "",
  activeSearchKey,
}: {
  hunkIndex: number;
  lines: DiffDto["hunks"][number]["lines"];
  selectedLines: Set<string>;
  onToggleLine: (key: string) => void;
  onBeginSelection: (
    key: string,
    event: ReactMouseEvent<HTMLButtonElement>,
  ) => void;
  onExtendSelection: (
    key: string,
    event: ReactMouseEvent<HTMLButtonElement>,
  ) => void;
  readOnly?: boolean;
  searchQuery?: string;
  activeSearchKey?: string;
}) {
  const rowHeight = 23;
  const [scrollTop, setScrollTop] = useState(0);
  const virtualListRef = useRef<HTMLDivElement>(null);
  const virtual = lines.length > 300;
  const normalizedSearch = searchQuery.toLocaleLowerCase();
  const activeLineIndex = activeSearchKey
    ? lines.findIndex(
        (line) => `${hunkIndex}:${line.index}` === activeSearchKey,
      )
    : -1;

  useEffect(() => {
    if (!virtual || activeLineIndex < 0) return;
    const nextScrollTop = Math.max(0, activeLineIndex * rowHeight - rowHeight * 8);
    setScrollTop(nextScrollTop);
    if (virtualListRef.current) {
      virtualListRef.current.scrollTop = nextScrollTop;
    }
  }, [activeLineIndex, virtual]);

  const start = virtual ? Math.max(0, Math.floor(scrollTop / rowHeight) - 8) : 0;
  const end = virtual ? Math.min(lines.length, start + 80) : lines.length;
  const content = lines.slice(start, end).map((line) => {
    const key = `${hunkIndex}:${line.index}`;
    const searchMatch =
      Boolean(normalizedSearch) &&
      line.content.toLocaleLowerCase().includes(normalizedSearch);
    const activeSearchMatch = key === activeSearchKey;
    return (
      <button
        type="button"
        key={key}
        className={[
          "diff-line",
          line.kind,
          selectedLines.has(key) ? "selected" : "",
          searchMatch ? "search-match" : "",
          activeSearchMatch ? "active-search-match" : "",
        ]
          .filter(Boolean)
          .join(" ")}
        data-diff-key={key}
        disabled={readOnly || !line.selectable}
        aria-pressed={!readOnly && line.selectable ? selectedLines.has(key) : undefined}
        onMouseDown={(event) =>
          !readOnly && line.selectable && onBeginSelection(key, event)
        }
        onMouseEnter={(event) =>
          !readOnly && line.selectable && onExtendSelection(key, event)
        }
        onClick={(event) => {
          if (!readOnly && line.selectable && event.detail === 0) onToggleLine(key);
        }}
      >
        <span>{line.oldLine ?? ""}</span>
        <span>{line.newLine ?? ""}</span>
        <code>
          <span className="diff-prefix">
            {line.kind === "addition" ? "+" : line.kind === "deletion" ? "-" : " "}
          </span>
          {highlightCodeLine(line.content)}
        </code>
      </button>
    );
  });
  if (!virtual) return <div className="diff-lines">{content}</div>;
  return (
    <div
      ref={virtualListRef}
      className="diff-lines virtual-diff"
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <div className="virtual-diff-space" style={{ height: lines.length * rowHeight }}>
        <div
          className="virtual-diff-window"
          style={{ transform: `translateY(${start * rowHeight}px)` }}
        >
          {content}
        </div>
      </div>
    </div>
  );
}

function ChangesEmpty({ onOpen, opening }: { onOpen: () => void; opening: boolean }) {
  return (
    <div className="changes-layout">
      <section className="file-panel" aria-label={t("Changed files")}>
        <ChangeSection
          title={t("Unstaged")}
          target="unstaged"
          changes={[]}
          onSelect={() => undefined}
        />
        <ChangeSection
          title={t("Staged")}
          target="staged"
          changes={[]}
          onSelect={() => undefined}
        />
      </section>
      <section className="welcome-panel">
        <div className="welcome-art" aria-hidden="true">
          <span className="branch-line" />
          <span className="branch-node node-one" />
          <span className="branch-node node-two" />
          <span className="branch-node node-three" />
        </div>
        <p className="eyebrow">{t("A calmer Git workflow")}</p>
        <h1>{t("Your repositories, clearly in view.")}</h1>
        <p>
          {t(
            "Open a local repository to inspect its real staged, unstaged, and untracked changes.",
          )}
        </p>
        <button
          className="control-button control-button--primary"
          type="button"
          onClick={onOpen}
          disabled={opening}
        >
          {opening ? t("Opening…") : t("Open a repository")}
        </button>
        <small>{t("Git 2.40.0 or newer is required.")}</small>
      </section>
    </div>
  );
}

function HistoryView({
  tab,
  snapshot,
  showGravatars,
  onPersist,
  onSnapshot,
  onError,
  onClearError,
}: {
  tab: SessionTabDto;
  snapshot: RepositorySnapshotDto;
  showGravatars: boolean;
  onPersist: (
    patch: Partial<
      Pick<SessionTabDto, "historyCursor" | "selectedCommit" | "historyFilter">
    >,
  ) => void;
  onSnapshot: (snapshot: RepositorySnapshotDto) => void;
  onError: (error: unknown) => void;
  onClearError: () => void;
}) {
  const savedFilter = parseHistoryFilter(tab.historyFilter);
  const [commits, setCommits] = useState<CommitDto[]>([]);
  const [query, setQuery] = useState(savedFilter.query);
  const [draftQuery, setDraftQuery] = useState(savedFilter.query);
  const [showHistorySearch, setShowHistorySearch] = useState(false);
  const [nextCursor, setNextCursor] = useState<string>();
  const [selectedOid, setSelectedOid] = useState(tab.selectedCommit);
  const [loading, setLoading] = useState(true);
  const [commitFiles, setCommitFiles] = useState<CommitFileDto[]>([]);
  const [showCommitFileSearch, setShowCommitFileSearch] = useState(false);
  const [commitFileFilter, setCommitFileFilter] = useState("");
  const [selectedFile, setSelectedFile] = useState<string>();
  const [commitFilesExpanded, setCommitFilesExpanded] = useState(true);
  const [commitDiff, setCommitDiff] = useState<DiffDto>();
  const [loadingCommitFiles, setLoadingCommitFiles] = useState(false);
  const [loadingCommitDiff, setLoadingCommitDiff] = useState(false);
  const [rebaseMenu, setRebaseMenu] = useState<{
    commit: CommitDto;
    x: number;
    y: number;
  }>();
  const [rebasePreview, setRebasePreview] =
    useState<InteractiveRebasePreviewDto>();
  const [rebasePlan, setRebasePlan] = useState<
    Array<{
      oid: string;
      subject: string;
      action: InteractiveRebaseAction;
      summary: string;
      description: string;
    }>
  >([]);
  const [autoStash, setAutoStash] = useState(false);
  const [rebaseBusy, setRebaseBusy] = useState(false);
  const draggedRebaseOid = useRef<string | undefined>(undefined);
  const [draggingRebaseOid, setDraggingRebaseOid] = useState<string>();
  const [rebaseDropTarget, setRebaseDropTarget] = useState<{
    oid: string;
    edge: "before" | "after";
  }>();
  const selectedRowRef = useRef<HTMLButtonElement | null>(null);
  const historySearchInputRef = useRef<HTMLInputElement>(null);
  const commitFileSearchInputRef = useRef<HTMLInputElement>(null);
  const selected = commits.find((commit) => commit.oid === selectedOid) ?? commits[0];
  const commitFileFilterQuery = commitFileFilter.trim().toLocaleLowerCase();
  const filteredCommitFiles = useMemo(
    () =>
      commitFiles.filter((file) =>
        file.path.toLocaleLowerCase().includes(commitFileFilterQuery),
      ),
    [commitFileFilterQuery, commitFiles],
  );
  const graph = useMemo(
    () =>
      layoutCommitGraph(
        query
          ? commits.map((commit) => ({ oid: commit.oid, parents: [] }))
          : commits,
      ),
    [commits, query],
  );
  const graphWidth = Math.max(44, graph.laneCount * 16 + 16);

  useEffect(() => {
    if (
      selectedRowRef.current &&
      typeof selectedRowRef.current.scrollIntoView === "function"
    ) {
      selectedRowRef.current.scrollIntoView({
        behavior: "smooth",
        block: "nearest",
      });
    }
  }, [selected?.oid, commits]);

  useEffect(() => {
    if (tab.selectedCommit) {
      setSelectedOid(tab.selectedCommit);
    }
  }, [tab.selectedCommit]);

  useEffect(() => {
    const parsed = parseHistoryFilter(tab.historyFilter);
    setQuery(parsed.query);
    setDraftQuery(parsed.query);
  }, [tab.historyFilter]);

  useEffect(() => {
    if (showHistorySearch) {
      historySearchInputRef.current?.focus();
    }
  }, [showHistorySearch]);

  useEffect(() => {
    const openSearch = () => setShowHistorySearch(true);
    window.addEventListener(openHistorySearchEvent, openSearch);
    return () => window.removeEventListener(openHistorySearchEvent, openSearch);
  }, []);

  useEffect(() => {
    const openSearch = () => {
      setShowCommitFileSearch(true);
      setCommitFilesExpanded(true);
    };
    window.addEventListener(openHistoryFileSearchEvent, openSearch);
    return () =>
      window.removeEventListener(openHistoryFileSearchEvent, openSearch);
  }, []);

  useEffect(() => {
    if (showCommitFileSearch) {
      commitFileSearchInputRef.current?.focus();
    }
  }, [showCommitFileSearch]);

  useEffect(() => {
    if (!rebaseMenu) return;
    const close = () => setRebaseMenu(undefined);
    window.addEventListener("click", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("blur", close);
    };
  }, [rebaseMenu]);

  useEffect(() => {
    let active = true;
    setCommitFiles([]);
    setShowCommitFileSearch(false);
    setCommitFileFilter("");
    setSelectedFile(undefined);
    setCommitFilesExpanded(true);
    setCommitDiff(undefined);
    if (!selected?.oid) return;
    setLoadingCommitFiles(true);
    getCommitFiles(snapshot.repository.id, selected.oid)
      .then((files) => {
        if (!active) return;
        setCommitFiles(files);
      })
      .catch(onError)
      .finally(() => active && setLoadingCommitFiles(false));
    return () => {
      active = false;
    };
  }, [snapshot.repository.id, selected?.oid]);

  useEffect(() => {
    let active = true;
    setCommitDiff(undefined);
    const file = commitFiles.find((item) => item.path === selectedFile);
    if (!selected?.oid || !file) return;
    setLoadingCommitDiff(true);
    getCommitDiff(snapshot.repository.id, selected.oid, file.pathBytes)
      .then((diff) => active && setCommitDiff(diff))
      .catch(onError)
      .finally(() => active && setLoadingCommitDiff(false));
    return () => {
      active = false;
    };
  }, [snapshot.repository.id, selected?.oid, selectedFile, commitFiles]);

  useEffect(() => {
    let active = true;
    getHistoryPage(
      snapshot.repository.id,
      undefined,
      undefined,
      query || undefined,
    )
      .then((page) => {
        if (!active) return;
        setCommits(page.commits);
        setNextCursor(page.nextCursor);
        const preferredOid = tab.selectedCommit || selectedOid;
        const matchedOid = page.commits.find((commit) => commit.oid === preferredOid)?.oid;
        const oid = matchedOid ?? preferredOid ?? page.commits[0]?.oid;
        setSelectedOid(oid);
        if (!tab.selectedCommit && oid) {
          onPersist({ selectedCommit: oid });
        }
      })
      .catch(onError)
      .finally(() => active && setLoading(false));
    return () => {
      active = false;
    };
  }, [
    snapshot.repository.id,
    snapshot.head.oid,
    query,
    tab.selectedCommit,
  ]);

  async function loadMore() {
    if (!nextCursor) return;
    setLoading(true);
    try {
      const cursor = nextCursor;
      const page = await getHistoryPage(
        snapshot.repository.id,
        cursor,
        undefined,
        query || undefined,
      );
      setCommits((current) => {
        const existing = new Set(current.map((commit) => commit.oid));
        return [
          ...current,
          ...page.commits.filter((commit) => !existing.has(commit.oid)),
        ];
      });
      setNextCursor(page.nextCursor);
    } catch (reason: unknown) {
      onError(reason);
    } finally {
      setLoading(false);
    }
  }

  function persistFilter(nextQuery: string) {
    onPersist({
      historyCursor: undefined,
      historyFilter: JSON.stringify({ query: nextQuery }),
    });
  }

  async function openInteractiveRebase(commit: CommitDto) {
    setRebaseMenu(undefined);
    onClearError();
    setRebaseBusy(true);
    try {
      const preview = await previewInteractiveRebase(
        snapshot.repository.id,
        snapshot.revision,
        commit.oid,
      );
      setRebasePreview(preview);
      setRebasePlan(
        preview.commits.map((item) => ({
          ...item,
          action: "pick" as const,
          summary: item.subject,
          description: "",
        })),
      );
      setAutoStash(snapshot.changes.length > 0);
    } catch (reason: unknown) {
      onError(reason);
    } finally {
      setRebaseBusy(false);
    }
  }

  function moveRebaseItem(index: number, offset: -1 | 1) {
    const destination = index + offset;
    if (destination < 0 || destination >= rebasePlan.length) return;
    setRebasePlan((current) => {
      const next = [...current];
      [next[index], next[destination]] = [next[destination], next[index]];
      return next;
    });
  }

  function finishRebaseDrag() {
    draggedRebaseOid.current = undefined;
    setDraggingRebaseOid(undefined);
    setRebaseDropTarget(undefined);
  }

  function dropRebaseItem(targetOid: string, edge: "before" | "after") {
    const sourceOid = draggedRebaseOid.current;
    if (!sourceOid || sourceOid === targetOid) {
      finishRebaseDrag();
      return;
    }
    setRebasePlan((current) => {
      const sourceIndex = current.findIndex((item) => item.oid === sourceOid);
      if (sourceIndex < 0) return current;
      const next = [...current];
      const [moved] = next.splice(sourceIndex, 1);
      const targetIndex = next.findIndex((item) => item.oid === targetOid);
      if (targetIndex < 0) return current;
      next.splice(targetIndex + (edge === "after" ? 1 : 0), 0, moved);
      return next;
    });
    finishRebaseDrag();
  }

  async function runInteractiveRebase() {
    if (!rebasePreview) return;
    onClearError();
    setRebaseBusy(true);
    try {
      const next = await startInteractiveRebase(
        snapshot.repository.id,
        snapshot.revision,
        {
          baseOid: rebasePreview.baseOid,
          expectedHeadOid: rebasePreview.headOid,
          items: rebasePlan.map(
            ({ oid, action, summary, description }) => ({
              oid,
              action,
              ...(action === "reword" ? { summary, description } : {}),
            }),
          ),
          autoStash,
        },
      );
      onSnapshot(next);
      setRebasePreview(undefined);
    } catch (reason: unknown) {
      onError(reason);
    } finally {
      setRebaseBusy(false);
    }
  }

  async function controlRebase(
    action: (
      repoId: string,
      revision: number,
    ) => Promise<RepositorySnapshotDto>,
  ) {
    onClearError();
    setRebaseBusy(true);
    try {
      onSnapshot(await action(snapshot.repository.id, snapshot.revision));
    } catch (reason: unknown) {
      onError(reason);
    } finally {
      setRebaseBusy(false);
    }
  }

  function closeHistorySearch() {
    setShowHistorySearch(false);
    setDraftQuery("");
    if (query) {
      setQuery("");
      persistFilter("");
    }
  }

  function closeCommitFileSearch() {
    setShowCommitFileSearch(false);
    setCommitFileFilter("");
  }

  function handleHistoryKeyDownCapture(
    event: ReactKeyboardEvent<HTMLElement>,
  ) {
    if (
      (event.ctrlKey || event.metaKey) &&
      event.key.toLocaleLowerCase() === "f"
    ) {
      event.preventDefault();
      event.stopPropagation();
      setShowHistorySearch(true);
      window.setTimeout(() => historySearchInputRef.current?.select(), 0);
      return;
    }
    if (event.key === "Escape" && showHistorySearch) {
      event.preventDefault();
      event.stopPropagation();
      closeHistorySearch();
    }
  }

  const hasConflicts = snapshot.changes.some((change) => change.conflict);

  return (
    <div className="history-view">
      <section
        className="history-list-panel"
        aria-label={t("Commit history")}
        onKeyDownCapture={handleHistoryKeyDownCapture}
      >
        {(snapshot.operation === "rebase" ||
          snapshot.operation === "rebaseEdit") && (
          <div className="rebase-status" role="status">
            <div>
              <strong>
                {snapshot.operation === "rebaseEdit"
                  ? t("Interactive rebase stopped for editing")
                  : t("Interactive rebase is paused")}
              </strong>
              <span>
                {hasConflicts
                  ? t("Resolve and stage conflicts, then continue.")
                  : snapshot.operation === "rebaseEdit"
                    ? t("Amend this commit in Changes, then continue the rebase.")
                  : t("Continue, skip the current commit, or abort the rebase.")}
              </span>
            </div>
            <div>
              <button
                className="control-button control-button--secondary"
                type="button"
                disabled={rebaseBusy || hasConflicts}
                onClick={() => void controlRebase(continueRebase)}
              >
                {t("Continue")}
              </button>
              <button
                className="control-button control-button--secondary"
                type="button"
                disabled={rebaseBusy}
                onClick={() => void controlRebase(skipRebase)}
              >
                {t("Skip")}
              </button>
              <button
                className="control-button control-button--danger"
                type="button"
                disabled={rebaseBusy}
                onClick={() => {
                  if (confirmRepositoryMutation(t("Abort this rebase and restore the previous branch state?"))) {
                    void controlRebase(abortRebase);
                  }
                }}
              >
                {t("Abort")}
              </button>
            </div>
          </div>
        )}
        {showHistorySearch && (
          <form
            className="history-filterbar"
            role="search"
            onSubmit={(event) => {
              event.preventDefault();
              const nextQuery = draftQuery.trim();
              setQuery(nextQuery);
              persistFilter(nextQuery);
            }}
          >
            <input
              ref={historySearchInputRef}
              className="control-input"
              type="search"
              aria-label={t("Search commit messages")}
              value={draftQuery}
              onChange={(event) => setDraftQuery(event.currentTarget.value)}
              placeholder={t("Search subject or body")}
            />
            <button
              className="control-button control-button--primary history-search-button"
              type="submit"
            >
              {t("Search")}
            </button>
            <button
              className="search-close-button"
              type="button"
              aria-label={t("Close history search")}
              onClick={closeHistorySearch}
            >
              ×
            </button>
          </form>
        )}
        {loading && commits.length === 0 ? (
          <div className="history-state" role="status">{t("Loading history…")}</div>
        ) : commits.length === 0 ? (
          <div className="history-state">{t("No commits match this filter.")}</div>
        ) : (
          <div
            className="commit-list"
            style={{ "--graph-width": `${graphWidth}px` } as CSSProperties}
          >
            {commits.map((commit, index) => (
              <button
                type="button"
                key={commit.oid}
                ref={selected?.oid === commit.oid ? selectedRowRef : undefined}
                className={[
                  "commit-row",
                  commit.remoteOnly ? "remote-only" : "",
                  selected?.oid === commit.oid ? "selected" : "",
                ].filter(Boolean).join(" ")}
                aria-current={selected?.oid === commit.oid ? "true" : undefined}
                onClick={() => {
                  setSelectedOid(commit.oid);
                  onPersist({ selectedCommit: commit.oid });
                }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  setSelectedOid(commit.oid);
                  onPersist({ selectedCommit: commit.oid });
                  setRebaseMenu({
                    commit,
                    x: event.clientX,
                    y: event.clientY,
                  });
                }}
                onKeyDown={(event) => {
                  if (
                    event.key === "ContextMenu" ||
                    (event.shiftKey && event.key === "F10")
                  ) {
                    event.preventDefault();
                    const bounds = event.currentTarget.getBoundingClientRect();
                    setRebaseMenu({
                      commit,
                      x: bounds.left + 16,
                      y: bounds.top + 16,
                    });
                  }
                }}
              >
                <CommitGraph
                  row={graph.rows[index]}
                  width={graphWidth}
                  label={t("Graph lane {lane} of {total}", {
                    lane: graph.rows[index].nodeLane + 1,
                    total: graph.rows[index].laneCount,
                  })}
                />
                <span className="commit-copy">
                  <strong>{commit.subject}</strong>
                  <span className="commit-author-line">
                    {showGravatars && (
                      <CommitAvatars
                        primaryAuthor={{
                          name: commit.authorName,
                          email: commit.authorEmail,
                        }}
                        coAuthors={coAuthorsFromCommitBody(commit.body)}
                      />
                    )}
                    <span>
                      {commit.authorName} · {relativeTime(commit.authoredAt)}
                    </span>
                  </span>
                </span>
                <span className="commit-refs">
                  {commit.references.slice(0, 2).map((item) => <small key={item}>{shortRef(item)}</small>)}
                </span>
                <code>{commit.oid.slice(0, 8)}</code>
              </button>
            ))}
          </div>
        )}
        {nextCursor && (
          <button className="load-more" type="button" disabled={loading} onClick={loadMore}>
            {loading ? t("Loading…") : t("Load older commits")}
          </button>
        )}
      </section>

      <aside className="commit-detail" aria-label={t("Commit details")}>
        {hasConflicts && snapshot.operation !== "rebase" && (
          <div className="merge-conflict" role="alert">
            {snapshot.operation === "autostashConflict"
              ? t("The rebase completed, but automatic stash restore has conflicts. Resolve them in Changes; the stash was kept.")
              : t("Merge stopped with conflicts. Resolve the highlighted files in Changes.")}
          </div>
        )}
        {selected ? (
          <>
            <div className="commit-metadata">
              <span className="eyebrow">{selected.oid}</span>
              <h1>{selected.subject}</h1>
              <p>{selected.authorName} &lt;{selected.authorEmail}&gt;</p>
              <time dateTime={new Date(selected.authoredAt * 1000).toISOString()}>
                {new Date(selected.authoredAt * 1000).toLocaleString(localeTag())}
              </time>
              {selected.body && <pre>{selected.body}</pre>}
              <div className="detail-refs">
                {selected.references.map((item) => <span key={item}>{shortRef(item)}</span>)}
              </div>
            </div>
            <section
              className="commit-files"
              aria-label={t("Files changed in commit")}
              onKeyDownCapture={(event) => {
                if (event.key === "Escape" && showCommitFileSearch) {
                  event.preventDefault();
                  event.stopPropagation();
                  closeCommitFileSearch();
                }
              }}
            >
              <button
                type="button"
                className="commit-files-toggle"
                aria-expanded={commitFilesExpanded}
                onClick={() => setCommitFilesExpanded((expanded) => !expanded)}
              >
                <span
                  className={`commit-files-chevron ${commitFilesExpanded ? "expanded" : ""}`}
                  aria-hidden="true"
                >
                  ›
                </span>
                <strong>{t("Changed files")}</strong>
                <small>
                  {commitFileFilterQuery
                    ? `${filteredCommitFiles.length}/${commitFiles.length}`
                    : commitFiles.length}
                </small>
              </button>
              {showCommitFileSearch && (
                <div className="panel-searchbar commit-file-searchbar" role="search">
                  <input
                    ref={commitFileSearchInputRef}
                    className="control-input"
                    type="search"
                    aria-label={t("Filter commit changed files")}
                    placeholder={t("Filter by file name or path")}
                    value={commitFileFilter}
                    onChange={(event) =>
                      setCommitFileFilter(event.currentTarget.value)
                    }
                  />
                  <button
                    className="search-close-button"
                    type="button"
                    aria-label={t("Close commit file filter")}
                    onClick={closeCommitFileSearch}
                  >
                    ×
                  </button>
                </div>
              )}
              {loadingCommitFiles ? (
                <div className="history-state" role="status">{t("Loading changed files…")}</div>
              ) : commitFiles.length === 0 ? (
                <div className="history-state">{t("No changed files.")}</div>
              ) : filteredCommitFiles.length === 0 ? (
                <div className="history-state">
                  {t("No changed files match this filter.")}
                </div>
              ) : commitFilesExpanded ? (
                <div className="commit-file-list">
                  {filteredCommitFiles.map((file) => {
                    const expanded = selectedFile === file.path;
                    return (
                      <div className="commit-file-entry" key={file.path}>
                        <button
                          type="button"
                          className={expanded ? "selected" : ""}
                          aria-expanded={expanded}
                          onClick={() =>
                            setSelectedFile((selected) =>
                              selected === file.path ? undefined : file.path,
                            )
                          }
                        >
                          <span
                            className={`commit-file-chevron ${expanded ? "expanded" : ""}`}
                            aria-hidden="true"
                          >
                            ›
                          </span>
                          <span>{file.path}</span>
                        </button>
                        {expanded && (
                          <section
                            className="commit-file-diff"
                            aria-label={t("Selected file changes")}
                          >
                            {loadingCommitDiff ? (
                              <div className="diff-state" role="status">
                                {t("Loading diff…")}
                              </div>
                            ) : commitDiff?.binary ? (
                              <div className="diff-state">
                                {t("Binary file preview is unavailable.")}
                              </div>
                            ) : commitDiff && commitDiff.hunks.length > 0 ? (
                              <DiffRenderer
                                diff={commitDiff}
                                selectedLines={new Set()}
                                onSelectionChange={() => undefined}
                                onToggleLine={() => undefined}
                                onApplyHunk={() => undefined}
                                actionLabel=""
                                actionDisabled
                                readOnly
                              />
                            ) : (
                              <div className="diff-state">
                                {t("No textual changes to display.")}
                              </div>
                            )}
                          </section>
                        )}
                      </div>
                    );
                  })}
                </div>
              ) : null}
            </section>
          </>
        ) : (
          <div className="history-state">{t("Select a commit to inspect it.")}</div>
        )}
      </aside>
      {rebaseMenu && (
        <div
          className="remote-context-menu"
          role="menu"
          aria-label={t("Commit actions")}
          style={{ left: rebaseMenu.x, top: rebaseMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            role="menuitem"
            disabled={
              rebaseBusy ||
              Boolean(snapshot.operation) ||
              snapshot.head.kind !== "branch" ||
              snapshot.head.oid === rebaseMenu.commit.oid ||
              Boolean(rebaseMenu.commit.remoteOnly)
            }
            onClick={() => void openInteractiveRebase(rebaseMenu.commit)}
          >
            {t("Interactively rebase commits after this…")}
          </button>
        </div>
      )}
      {rebasePreview && (
        <div
          className="modal-overlay"
          role="presentation"
          onClick={() => !rebaseBusy && setRebasePreview(undefined)}
        >
          <div
            className="settings-modal interactive-rebase-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="interactive-rebase-title"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="settings-modal-header">
              <div>
                <h2 id="interactive-rebase-title">{t("Interactive rebase")}</h2>
                <small>
                  {rebasePreview.branch} · {rebasePreview.baseOid.slice(0, 8)}..HEAD
                </small>
              </div>
              <button
                className="settings-close-btn"
                type="button"
                aria-label={t("Close interactive rebase")}
                disabled={rebaseBusy}
                onClick={() => setRebasePreview(undefined)}
              >
                ×
              </button>
            </div>
            <div className="interactive-rebase-body">
              <p>
                {t("Reorder commits and choose how each commit should be replayed. Merge commits are not supported yet.")}
              </p>
              <ol className="interactive-rebase-list">
                {rebasePlan.map((item, index) => (
                  <li
                    key={item.oid}
                    className={[
                      draggingRebaseOid === item.oid ? "dragging" : "",
                      rebaseDropTarget?.oid === item.oid
                        ? `drop-${rebaseDropTarget.edge}`
                        : "",
                    ].filter(Boolean).join(" ")}
                    draggable={!rebaseBusy}
                    tabIndex={0}
                    aria-label={t("Drag to reorder {subject}", {
                      subject: item.subject,
                    })}
                    onDragStart={(event) => {
                      if (
                        (event.target as HTMLElement).closest(
                          "input, textarea, select",
                        )
                      ) {
                        event.preventDefault();
                        return;
                      }
                      draggedRebaseOid.current = item.oid;
                      setDraggingRebaseOid(item.oid);
                      event.dataTransfer.effectAllowed = "move";
                      event.dataTransfer.setData("text/plain", item.oid);
                    }}
                    onDragOver={(event) => {
                      if (
                        !draggedRebaseOid.current ||
                        draggedRebaseOid.current === item.oid
                      ) {
                        return;
                      }
                      event.preventDefault();
                      event.dataTransfer.dropEffect = "move";
                      const bounds = event.currentTarget.getBoundingClientRect();
                      setRebaseDropTarget({
                        oid: item.oid,
                        edge:
                          event.clientY < bounds.top + bounds.height / 2
                            ? "before"
                            : "after",
                      });
                    }}
                    onDrop={(event) => {
                      event.preventDefault();
                      const edge =
                        rebaseDropTarget?.oid === item.oid
                          ? rebaseDropTarget.edge
                          : "after";
                      dropRebaseItem(item.oid, edge);
                    }}
                    onDragEnd={finishRebaseDrag}
                    onKeyDown={(event) => {
                      if (event.target !== event.currentTarget) return;
                      if (event.altKey && event.key === "ArrowUp") {
                        event.preventDefault();
                        moveRebaseItem(index, -1);
                      } else if (event.altKey && event.key === "ArrowDown") {
                        event.preventDefault();
                        moveRebaseItem(index, 1);
                      }
                    }}
                  >
                    <span className="rebase-drag-handle" aria-hidden="true">
                      ⠿
                    </span>
                    <select
                      className="control-input rebase-action-select"
                      draggable={false}
                      aria-label={t("Action for {subject}", { subject: item.subject })}
                      value={item.action}
                      disabled={rebaseBusy}
                      onDragStart={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                      }}
                      onChange={(event) => {
                        const action = event.currentTarget
                          .value as InteractiveRebaseAction;
                        setRebasePlan((current) =>
                          current.map((entry, entryIndex) =>
                            entryIndex === index ? { ...entry, action } : entry,
                          ),
                        );
                      }}
                    >
                      <option value="pick">pick</option>
                      <option value="reword">reword</option>
                      <option value="edit">edit</option>
                      <option value="squash">squash</option>
                      <option value="fixup">fixup</option>
                      <option value="drop">drop</option>
                    </select>
                    <code>{item.oid.slice(0, 8)}</code>
                    <span className="rebase-commit-subject">{item.subject}</span>
                    {item.action === "reword" && (
                      <div className="rebase-message-fields">
                        <input
                          className="control-input"
                          type="text"
                          aria-label={t("New summary for {subject}", {
                            subject: item.subject,
                          })}
                          value={item.summary}
                          disabled={rebaseBusy}
                          onDragStart={(event) => {
                            event.preventDefault();
                            event.stopPropagation();
                          }}
                          onChange={(event) => {
                            const summary = event.currentTarget.value;
                            setRebasePlan((current) =>
                              current.map((entry, entryIndex) =>
                                entryIndex === index
                                  ? { ...entry, summary }
                                  : entry,
                              ),
                            );
                          }}
                          placeholder={t("New commit summary")}
                        />
                        <textarea
                          className="control-input"
                          aria-label={t("New description for {subject}", {
                            subject: item.subject,
                          })}
                          value={item.description}
                          disabled={rebaseBusy}
                          onDragStart={(event) => {
                            event.preventDefault();
                            event.stopPropagation();
                          }}
                          onChange={(event) => {
                            const description = event.currentTarget.value;
                            setRebasePlan((current) =>
                              current.map((entry, entryIndex) =>
                                entryIndex === index
                                  ? { ...entry, description }
                                  : entry,
                              ),
                            );
                          }}
                          placeholder={t("New commit description (optional)")}
                        />
                      </div>
                    )}
                    {item.action === "edit" && (
                      <small className="rebase-action-note">
                        {t("Rebase will stop at this commit so you can amend its contents.")}
                      </small>
                    )}
                  </li>
                ))}
              </ol>
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={autoStash}
                  disabled={rebaseBusy}
                  onChange={(event) => setAutoStash(event.currentTarget.checked)}
                />
                <span>{t("Automatically stash and reapply local changes")}</span>
              </label>
              {snapshot.changes.length > 0 && !autoStash && (
                <small className="rebase-warning">
                  {t("The working tree has changes. Enable automatic stash to continue.")}
                </small>
              )}
              <div className="remote-form-actions">
                <button
                  className="control-button control-button--secondary"
                  type="button"
                  disabled={rebaseBusy}
                  onClick={() => setRebasePreview(undefined)}
                >
                  {t("Cancel")}
                </button>
                <button
                  type="button"
                  className="control-button control-button--primary"
                  disabled={
                    rebaseBusy ||
                    (snapshot.changes.length > 0 && !autoStash) ||
                    rebasePlan.every((item) => item.action === "drop") ||
                    rebasePlan.some(
                      (item) =>
                        item.action === "reword" && !item.summary.trim(),
                    ) ||
                    rebasePlan.some(
                      (item, index) =>
                        (item.action === "squash" || item.action === "fixup") &&
                        !rebasePlan
                          .slice(0, index)
                          .some((previous) => previous.action !== "drop"),
                    )
                  }
                  onClick={() => void runInteractiveRebase()}
                >
                  {rebaseBusy ? t("Rebasing…") : t("Start rebase")}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function CommitAvatars({
  primaryAuthor,
  coAuthors,
}: {
  primaryAuthor: GravatarAuthor;
  coAuthors: GravatarAuthor[];
}) {
  const primaryEmail = primaryAuthor.email.trim().toLowerCase();
  const authors = [
    primaryAuthor,
    ...coAuthors.filter(
      (author) => author.email.trim().toLowerCase() !== primaryEmail,
    ),
  ];

  return (
    <span className="commit-avatars">
      {authors.map((author, index) => (
        <GravatarAvatar
          key={author.email.trim().toLowerCase()}
          authorName={author.name}
          email={author.email}
          stackOrder={authors.length - index}
        />
      ))}
    </span>
  );
}

function GravatarAvatar({
  authorName,
  email,
  stackOrder,
}: {
  authorName: string;
  email: string;
  stackOrder: number;
}) {
  const [source, setSource] = useState<string>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    setSource(undefined);
    setFailed(false);
    gravatarUrl(email)
      .then((url) => active && setSource(url))
      .catch(() => active && setFailed(true));
    return () => {
      active = false;
    };
  }, [email]);

  if (!source || failed) return null;
  return (
    <img
      className="commit-avatar"
      src={source}
      alt=""
      title={t("{name}'s Gravatar", { name: authorName })}
      style={{ zIndex: stackOrder }}
      onError={() => setFailed(true)}
    />
  );
}

const GRAPH_ROW_HEIGHT = 56;
const GRAPH_NODE_Y = GRAPH_ROW_HEIGHT / 2;
const GRAPH_LANE_GAP = 16;
const GRAPH_LANE_OFFSET = 12;

function CommitGraph({
  row,
  width,
  label,
}: {
  row: CommitGraphRow;
  width: number;
  label: string;
}) {
  const laneX = (lane: number) => GRAPH_LANE_OFFSET + lane * GRAPH_LANE_GAP;

  return (
    <svg
      className="commit-graph"
      viewBox={`0 0 ${width} ${GRAPH_ROW_HEIGHT}`}
      role="img"
      aria-label={label}
    >
      {row.segments.map((segment, index) => (
        <path
          key={`${segment.from}-${segment.to}-${segment.fromLane}-${segment.toLane}-${index}`}
          className={`graph-edge graph-color-${segment.color % 8}`}
          d={graphSegmentPath(segment, laneX)}
        />
      ))}
      <circle
        className={`graph-commit-node graph-color-${row.nodeColor % 8}`}
        cx={laneX(row.nodeLane)}
        cy={GRAPH_NODE_Y}
        r="5"
      />
    </svg>
  );
}

function graphSegmentPath(
  segment: GraphSegment,
  laneX: (lane: number) => number,
) {
  const startY = segment.from === "top" ? 0 : GRAPH_NODE_Y;
  const endY = segment.to === "node" ? GRAPH_NODE_Y : GRAPH_ROW_HEIGHT;
  const startX = laneX(segment.fromLane);
  const endX = laneX(segment.toLane);
  const middleY = (startY + endY) / 2;
  return `M ${startX} ${startY} C ${startX} ${middleY}, ${endX} ${middleY}, ${endX} ${endY}`;
}

function runWindowCommand(command: () => Promise<void>) {
  void command().catch((reason: unknown) => {
    console.error("Window command failed", reason);
  });
}

function parseHistoryFilter(value?: string): { reference: string; query: string } {
  if (!value) return { reference: "", query: "" };
  try {
    const parsed = JSON.parse(value) as { reference?: string; query?: string };
    return { reference: parsed.reference ?? "", query: parsed.query ?? "" };
  } catch {
    return { reference: "", query: value };
  }
}

function shortRef(value: string) {
  return value
    .replace("HEAD -> ", "")
    .replace("refs/heads/", "")
    .replace("refs/remotes/", "")
    .replace("refs/tags/", "");
}

function relativeTime(timestamp: number) {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000) - timestamp);
  if (seconds < 60) return t("just now");
  if (seconds < 3600) return t("{count}m ago", { count: Math.floor(seconds / 60) });
  if (seconds < 86_400) return t("{count}h ago", { count: Math.floor(seconds / 3600) });
  if (seconds < 2_592_000) return t("{count}d ago", { count: Math.floor(seconds / 86_400) });
  return new Date(timestamp * 1000).toLocaleDateString(localeTag());
}

function operationTerm(value: string): string {
  switch (value.toLowerCase()) {
    case "fetch": return t("Fetch");
    case "pull": return t("Pull");
    case "push": return t("Push");
    case "clone": return t("clone");
    case "queued": return t("queued");
    case "running": return t("running");
    case "succeeded": return t("succeeded");
    case "failed": return t("failed");
    case "cancelled": return t("cancelled");
    case "interrupted": return t("interrupted");
    default: return value;
  }
}

function HistoryEmpty() {
  return <div className="history-empty"><div className="history-lines" aria-hidden="true"><i /><i /><i /></div><p className="eyebrow">{t("Commit graph")}</p><h1>{t("History will appear here.")}</h1><p>{t("Open a repository to explore commits, branches, tags, and authors.")}</p></div>;
}
