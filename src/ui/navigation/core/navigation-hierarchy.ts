export interface NavigationRegionConfig {
  regionId: string;
  parentRegionId?: string;
  childRegionId?: string;
  entryFocusId?: string;
  exitFocusId?: string;
  gamepadParentRegionId?: string;
  gamepadExitFocusId?: string;
}

export interface NavigationRegionEntry extends NavigationRegionConfig {
  focusId: string;
  disabled?: boolean;
  hidden?: boolean;
}

export interface NavigationHierarchySnapshot {
  activeChildRegionByParent: Record<string, string>;
  lastFocusedByRegion: Record<string, string>;
  preferredItemIndexByRegion: Record<string, number>;
}

function isValid(entry: NavigationRegionEntry): boolean {
  return !entry.disabled && !entry.hidden;
}

export class NavigationLevelCoordinator {
  private readonly activeChildRegionByParent = new Map<string, string>();
  private readonly lastFocusedByRegion = new Map<string, string>();
  private readonly preferredItemIndexByRegion = new Map<string, number>();

  public reset(): void {
    this.activeChildRegionByParent.clear();
    this.lastFocusedByRegion.clear();
    this.preferredItemIndexByRegion.clear();
  }

  public recordFocus(
    config: NavigationRegionConfig,
    focusId: string,
    preferredItemIndex?: number,
  ): void {
    this.lastFocusedByRegion.set(config.regionId, focusId);
    if (preferredItemIndex !== undefined) {
      this.preferredItemIndexByRegion.set(config.regionId, preferredItemIndex);
    }
    if (config.parentRegionId) {
      this.activeChildRegionByParent.set(
        config.parentRegionId,
        config.regionId,
      );
    }
  }

  public resolveChild(
    parent: NavigationRegionConfig,
    entries: readonly NavigationRegionEntry[],
  ): string | null {
    const childRegionId = parent.childRegionId;
    if (!childRegionId) return null;
    const childEntries = entries.filter(
      (entry) => entry.regionId === childRegionId && isValid(entry),
    );
    if (childEntries.length === 0) {
      return this.lastFocusedByRegion.get(childRegionId) ?? null;
    }

    const lastFocused = this.lastFocusedByRegion.get(childRegionId);
    if (
      lastFocused &&
      childEntries.some((entry) => entry.focusId === lastFocused)
    ) {
      return lastFocused;
    }

    if (
      parent.entryFocusId &&
      childEntries.some((entry) => entry.focusId === parent.entryFocusId)
    ) {
      return parent.entryFocusId;
    }

    return childEntries[0]?.focusId ?? null;
  }

  public resolveParent(
    child: NavigationRegionConfig,
    entries: readonly NavigationRegionEntry[],
  ): string | null {
    if (child.exitFocusId) {
      const explicitExit = entries.find(
        (entry) => entry.focusId === child.exitFocusId && isValid(entry),
      );
      if (explicitExit) return explicitExit.focusId;
    }
    if (!child.parentRegionId) return null;

    const parentEntries = entries.filter(
      (entry) => entry.regionId === child.parentRegionId && isValid(entry),
    );
    const lastParentFocus = this.lastFocusedByRegion.get(child.parentRegionId);
    if (
      lastParentFocus &&
      parentEntries.some((entry) => entry.focusId === lastParentFocus)
    ) {
      return lastParentFocus;
    }
    return parentEntries[0]?.focusId ?? null;
  }

  public getLastFocusedFocusId(regionId: string): string | null {
    return this.lastFocusedByRegion.get(regionId) ?? null;
  }

  public getPreferredItemIndex(regionId: string): number | null {
    return this.preferredItemIndexByRegion.get(regionId) ?? null;
  }

  public getSnapshot(): NavigationHierarchySnapshot {
    return {
      activeChildRegionByParent: Object.fromEntries(
        this.activeChildRegionByParent,
      ),
      lastFocusedByRegion: Object.fromEntries(this.lastFocusedByRegion),
      preferredItemIndexByRegion: Object.fromEntries(
        this.preferredItemIndexByRegion,
      ),
    };
  }
}
