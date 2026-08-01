export class FocusHistory {
  private readonly history = new Map<string, string>();

  public remember(scopeId: string, focusId: string | null): void {
    if (focusId) this.history.set(scopeId, focusId);
  }

  public get(scopeId: string): string | undefined {
    return this.history.get(scopeId);
  }

  public clear(scopeId: string): void {
    this.history.delete(scopeId);
  }
}
