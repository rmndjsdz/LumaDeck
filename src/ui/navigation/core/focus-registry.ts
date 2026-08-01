import type { FocusEntry, Rect } from "./navigation-types";

type RegistryListener = () => void;

export class FocusRegistry {
  private readonly entries = new Map<string, FocusEntry>();
  private readonly rectCache = new Map<string, Rect>();
  private readonly listeners = new Set<RegistryListener>();
  private readonly duplicateIds = new Set<string>();
  private readonly resizeObserver: ResizeObserver | null;

  public constructor() {
    this.resizeObserver =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(() => this.invalidateAll());
  }

  public register(entry: FocusEntry): () => void {
    const existing = this.entries.get(entry.focusId);
    if (existing && existing.element !== entry.element) {
      this.duplicateIds.add(entry.focusId);
      this.notify();
      throw new Error(`Duplicate focusId detected: ${entry.focusId}`);
    }

    this.entries.set(entry.focusId, entry);
    this.rectCache.delete(entry.focusId);
    this.resizeObserver?.observe(entry.element);
    this.notify();

    return () => {
      if (this.entries.get(entry.focusId)?.element === entry.element) {
        this.unregister(entry.focusId);
      }
    };
  }

  public unregister(focusId: string): void {
    const entry = this.entries.get(focusId);
    if (!entry) return;
    this.resizeObserver?.unobserve(entry.element);
    this.entries.delete(focusId);
    this.rectCache.delete(focusId);
    this.notify();
  }

  public update(
    focusId: string,
    patch: Partial<Omit<FocusEntry, "focusId" | "element">>,
  ): void {
    const entry = this.entries.get(focusId);
    if (!entry) return;
    Object.assign(entry, patch);
    this.rectCache.delete(focusId);
  }

  public get(focusId: string): FocusEntry | undefined {
    return this.entries.get(focusId);
  }

  public count(): number {
    return this.entries.size;
  }

  public getScopeEntries(scopeId: string): FocusEntry[] {
    return [...this.entries.values()].filter(
      (entry) =>
        entry.scopeId === scopeId &&
        !entry.disabled &&
        !entry.hidden &&
        entry.element.isConnected,
    );
  }

  public getRect(entry: FocusEntry): Rect {
    const cached = this.rectCache.get(entry.focusId);
    if (cached) return cached;
    const rect = entry.element.getBoundingClientRect();
    const normalized: Rect = {
      top: rect.top,
      right: rect.right,
      bottom: rect.bottom,
      left: rect.left,
      width: rect.width,
      height: rect.height,
    };
    this.rectCache.set(entry.focusId, normalized);
    return normalized;
  }

  public invalidateAll(): void {
    this.rectCache.clear();
  }

  public subscribe(listener: RegistryListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  public getDuplicateIds(): string[] {
    return [...this.duplicateIds];
  }

  private notify(): void {
    for (const listener of this.listeners) listener();
  }
}
