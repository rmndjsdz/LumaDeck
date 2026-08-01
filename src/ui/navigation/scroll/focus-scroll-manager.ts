export interface ScrollResult {
  scrolled: boolean;
  focusId: string;
}

export class FocusScrollManager {
  private readonly positions = new Map<string, { top: number; left: number }>();

  public ensureVisible(element: HTMLElement, focusId: string): ScrollResult {
    const rect = element.getBoundingClientRect();
    const viewportHeight = window.innerHeight;
    const viewportWidth = window.innerWidth;
    const visible =
      rect.top >= 0 &&
      rect.left >= 0 &&
      rect.bottom <= viewportHeight &&
      rect.right <= viewportWidth;

    if (!visible && typeof element.scrollIntoView === "function") {
      element.scrollIntoView({ block: "nearest", inline: "nearest" });
    }

    return { scrolled: !visible, focusId };
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
}
