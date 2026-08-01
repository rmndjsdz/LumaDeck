import { useLayoutEffect, useMemo, useRef, type MutableRefObject } from "react";

import { useNavigation } from "../navigation-context";
import { useNavigationStore } from "../../../stores/navigation-store";
import type {
  FocusNavigationOverrides,
  InputMode,
} from "../core/navigation-types";
import {
  useGridNavigation,
  useLinearNavigation,
  useRowNavigation,
} from "../layouts/linear-navigation-context";

export interface UseFocusableOptions {
  focusId: string;
  scopeId: string;
  groupId?: string;
  itemIndex?: number;
  gridIndex?: number;
  disabled?: boolean;
  hidden?: boolean;
  navigation?: FocusNavigationOverrides;
  onFocus?: () => void;
  onBlur?: () => void;
  onConfirm?: () => void;
  priority?: number;
}

export interface UseFocusableResult<T extends HTMLElement> {
  ref: MutableRefObject<T | null>;
  isActive: boolean;
  inputMode: InputMode;
  tabIndex: 0 | -1;
  onMouseEnter: () => void;
  onClick: () => void;
}

export function useFocusable<T extends HTMLElement = HTMLElement>(
  options: UseFocusableOptions,
): UseFocusableResult<T> {
  const runtime = useNavigation();
  const ref = useRef<T | null>(null);
  const optionsRef = useRef(options);
  optionsRef.current = options;
  const isActive = useNavigationStore(
    (state) => state.activeFocusId === options.focusId,
  );
  const inputMode = useNavigationStore((state) => state.inputMode);
  const linearNavigation = useLinearNavigation();
  const rowNavigationContext = useRowNavigation();
  const rowGroupId = rowNavigationContext?.groupId;
  const rowId = rowNavigationContext?.rowId;
  const rowIndex = rowNavigationContext?.rowIndex;
  const preserveHorizontalIntent =
    rowNavigationContext?.preserveHorizontalIntent;
  const gridNavigation = useGridNavigation();
  const rowNavigation = useMemo(
    () =>
      rowGroupId &&
      rowId &&
      rowIndex !== undefined &&
      options.itemIndex !== undefined
        ? {
            groupId: rowGroupId,
            rowId,
            rowIndex,
            itemIndex: options.itemIndex,
            preserveHorizontalIntent: preserveHorizontalIntent ?? false,
          }
        : undefined,
    [options.itemIndex, preserveHorizontalIntent, rowGroupId, rowId, rowIndex],
  );

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const current = () => optionsRef.current;
    return runtime.registry.register({
      focusId: options.focusId,
      scopeId: options.scopeId,
      groupId:
        options.groupId ?? linearNavigation?.groupId ?? gridNavigation?.groupId,
      disabled: options.disabled,
      hidden: options.hidden,
      navigation: current().navigation,
      linearNavigation: linearNavigation ?? undefined,
      rowNavigation,
      gridNavigation:
        gridNavigation && options.gridIndex !== undefined
          ? { ...gridNavigation, index: options.gridIndex }
          : (gridNavigation ?? undefined),
      priority: options.priority,
      element,
      onFocus: () => current().onFocus?.(),
      onBlur: () => current().onBlur?.(),
      onConfirm: () => current().onConfirm?.(),
    });
  }, [
    options.disabled,
    options.focusId,
    options.groupId,
    options.itemIndex,
    options.gridIndex,
    options.hidden,
    options.priority,
    options.scopeId,
    linearNavigation,
    linearNavigation?.axis,
    linearNavigation?.groupId,
    linearNavigation?.wrap,
    rowNavigation,
    rowNavigation?.groupId,
    rowNavigation?.itemIndex,
    rowNavigation?.rowId,
    rowNavigation?.rowIndex,
    rowNavigation?.preserveHorizontalIntent,
    gridNavigation,
    gridNavigation?.columns,
    gridNavigation?.groupId,
    runtime.registry,
  ]);

  return {
    ref,
    isActive,
    inputMode,
    tabIndex: options.disabled || !isActive ? -1 : 0,
    onMouseEnter: () =>
      runtime.inputManager.handlePointerHover(options.focusId),
    onClick: () => runtime.inputManager.handlePointerConfirm(options.focusId),
  };
}
