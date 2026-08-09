export interface ScrollResult {
  scrolled: boolean;
  focusId: string;
}

export class FocusScrollManager {
  private readonly positions = new Map<string, { top: number; left: number }>();

  public ensureVisible(element: HTMLElement, focusId: string): ScrollResult {
    const rect = element.getBoundingClientRect();
    const visible = this.isVisibleInViewportAndScrollAncestors(element, rect);

    if (!visible && typeof element.scrollIntoView === "function") {
      const phase =
        typeof document !== "undefined"
          ? document.documentElement.dataset.navigationPhase
          : undefined;
      element.scrollIntoView({
        block: "nearest",
        inline: "nearest",
        behavior: phase === "fast-navigating" ? "auto" : "smooth",
      });
    }

    return { scrolled: !visible, focusId };
  }

  private isVisibleInViewportAndScrollAncestors(
    element: HTMLElement,
    rect: DOMRect,
  ): boolean {
    const visibleInViewport =
      rect.top >= 0 &&
      rect.left >= 0 &&
      rect.bottom <= window.innerHeight &&
      rect.right <= window.innerWidth;
    if (!visibleInViewport) return false;

    let ancestor = element.parentElement;
    while (ancestor) {
      const style = window.getComputedStyle(ancestor);
      const scrollable =
        /(auto|scroll|overlay)/.test(style.overflowY) &&
        ancestor.scrollHeight > ancestor.clientHeight;
      if (scrollable) {
        const ancestorRect = ancestor.getBoundingClientRect();
        if (
          rect.top < ancestorRect.top ||
          rect.bottom > ancestorRect.bottom ||
          rect.left < ancestorRect.left ||
          rect.right > ancestorRect.right
        ) {
          return false;
        }
      }
      ancestor = ancestor.parentElement;
    }
    return true;
  }

  public remember(scopeId: string, element: HTMLElement): void {
    this.positions.set(scopeId, {
      top: element.scrollTop,
      left: element.scrollLeft,
    });
  }

  public restore(scopeId: string, element: HTMLElement): boolean {
    const position = this.positions.get(scopeId);
    if (!position) return false;
    element.scrollTop = position.top;
    element.scrollLeft = position.left;
    return true;
  }

  public clear(scopeId: string): void {
    this.positions.delete(scopeId);
  }

  public rememberScope(scopeId: string): void {
    const element = this.findScopeElement(scopeId);
    if (element) this.remember(scopeId, element);
  }

  public restoreScope(scopeId: string): boolean {
    const element = this.findScopeElement(scopeId);
    return element ? this.restore(scopeId, element) : false;
  }

  private findScopeElement(scopeId: string): HTMLElement | undefined {
    if (typeof document === "undefined") return undefined;
    return [
      ...document.querySelectorAll<HTMLElement>("[data-scroll-scope]"),
    ].find((element) => element.dataset.scrollScope === scopeId);
  }
}
