import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";

import { Focusable } from "../../ui/navigation/focus/Focusable";
import { MediaImage } from "../../ui/performance/MediaImage";
import { NavigationDialog } from "../../ui/navigation/layouts/NavigationDialog";
import { useNavigation } from "../../ui/navigation/navigation-context";
import type { NavigationAction } from "../../ui/navigation/core/navigation-types";
import {
  clampScreenshotPan,
  getCircularScreenshotIndex,
  getZoomAfterStep,
  SCREENSHOT_DEFAULT_PAN,
  SCREENSHOT_DEFAULT_ZOOM,
  type ScreenshotPan,
  type ScreenshotZoom,
} from "./screenshot-viewer-state";

const VIEWER_SCOPE_ID = "details-screenshot-viewer";
const IMAGE_FOCUS_ID = "details-screenshot-viewer-image";
const PREVIOUS_FOCUS_ID = "details-screenshot-viewer-previous";
const NEXT_FOCUS_ID = "details-screenshot-viewer-next";
const FULLSCREEN_FOCUS_ID = "details-screenshot-viewer-fullscreen";
const CONTROL_HIDE_DELAY_MS = 2400;
const IDLE_PULSE_DELAY_MS = 1600;
const CLOSE_DURATION_MS = 192;
const PAN_STEP = 64;

export interface ScreenshotViewerOrigin {
  left: number;
  top: number;
  width: number;
  height: number;
  borderRadius: string;
  boxShadow: string;
}

type ViewerRect = ScreenshotViewerOrigin;

export function ScreenshotViewer({
  gameTitle,
  gameId,
  screenshots,
  initialIndex,
  origin,
  onClose,
}: {
  gameTitle: string;
  gameId: string;
  screenshots: readonly string[];
  initialIndex: number;
  origin: ScreenshotViewerOrigin | null;
  onClose: () => void;
}) {
  const { engine } = useNavigation();
  const stageRef = useRef<HTMLDivElement | null>(null);
  const imageRef = useRef<HTMLImageElement | null>(null);
  const hideControlsTimerRef = useRef<number | null>(null);
  const closeTimerRef = useRef<number | null>(null);
  const idleTimerRef = useRef<number | null>(null);
  const transitionFrameRef = useRef<number | null>(null);
  const [currentIndex, setCurrentIndex] = useState(initialIndex);
  const [zoom, setZoom] = useState<ScreenshotZoom>(SCREENSHOT_DEFAULT_ZOOM);
  const [pan, setPan] = useState<ScreenshotPan>(SCREENSHOT_DEFAULT_PAN);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [isIdle, setIsIdle] = useState(false);
  const [isClosing, setIsClosing] = useState(false);
  const [isOpening, setIsOpening] = useState(true);
  const [targetRect, setTargetRect] = useState<ViewerRect | null>(null);
  const [imageLoadVersion, setImageLoadVersion] = useState(0);

  const clearHideControlsTimer = useCallback(() => {
    if (hideControlsTimerRef.current === null) return;
    window.clearTimeout(hideControlsTimerRef.current);
    hideControlsTimerRef.current = null;
  }, []);

  const clearIdleTimer = useCallback(() => {
    if (idleTimerRef.current === null) return;
    window.clearTimeout(idleTimerRef.current);
    idleTimerRef.current = null;
  }, []);

  const markInteraction = useCallback(() => {
    setControlsVisible(true);
    setIsIdle(false);
    clearIdleTimer();
    idleTimerRef.current = window.setTimeout(() => {
      idleTimerRef.current = null;
      setIsIdle(true);
    }, IDLE_PULSE_DELAY_MS);
  }, [clearIdleTimer]);

  useEffect(() => {
    clearHideControlsTimer();
    if (!controlsVisible) return;
    hideControlsTimerRef.current = window.setTimeout(() => {
      hideControlsTimerRef.current = null;
      setControlsVisible(false);
    }, CONTROL_HIDE_DELAY_MS);
    return clearHideControlsTimer;
  }, [clearHideControlsTimer, controlsVisible, currentIndex, pan, zoom]);

  useEffect(() => {
    markInteraction();
    return () => {
      clearHideControlsTimer();
      clearIdleTimer();
      if (closeTimerRef.current !== null) {
        window.clearTimeout(closeTimerRef.current);
      }
      if (transitionFrameRef.current !== null) {
        window.cancelAnimationFrame(transitionFrameRef.current);
      }
    };
  }, [clearHideControlsTimer, clearIdleTimer, markInteraction]);

  useEffect(() => {
    const urls = [
      screenshots[currentIndex],
      screenshots[
        getCircularScreenshotIndex(currentIndex, screenshots.length, -1)
      ],
      screenshots[
        getCircularScreenshotIndex(currentIndex, screenshots.length, 1)
      ],
    ].filter((url): url is string => Boolean(url));
    urls.forEach((url) => {
      const image = new Image();
      image.src = url;
    });
  }, [currentIndex, screenshots]);

  const measureTargetRect = useCallback((): ViewerRect | null => {
    const stage = stageRef.current;
    if (!stage) return null;
    const stageRect = stage.getBoundingClientRect();
    const stageStyle = window.getComputedStyle(stage);
    return {
      left: stageRect.left,
      top: stageRect.top,
      width: stageRect.width,
      height: stageRect.height,
      borderRadius: stageStyle.borderRadius,
      boxShadow: stageStyle.boxShadow,
    };
  }, []);

  useLayoutEffect(() => {
    transitionFrameRef.current = window.requestAnimationFrame(() => {
      transitionFrameRef.current = null;
      setTargetRect(measureTargetRect());
      setIsOpening(false);
    });
    return () => {
      if (transitionFrameRef.current !== null) {
        window.cancelAnimationFrame(transitionFrameRef.current);
        transitionFrameRef.current = null;
      }
    };
  }, [imageLoadVersion, isFullscreen, measureTargetRect]);

  useEffect(() => {
    const handleResize = () => setTargetRect(measureTargetRect());
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [measureTargetRect]);

  const resetViewport = useCallback(() => {
    setZoom(SCREENSHOT_DEFAULT_ZOOM);
    setPan(SCREENSHOT_DEFAULT_PAN);
  }, []);

  const changeScreenshot = useCallback(
    (delta: -1 | 1) => {
      setCurrentIndex((index) =>
        getCircularScreenshotIndex(index, screenshots.length, delta),
      );
      resetViewport();
      markInteraction();
    },
    [markInteraction, resetViewport, screenshots.length],
  );

  const getPanBounds = useCallback((): ScreenshotPan => {
    const stage = stageRef.current;
    const image = imageRef.current;
    if (!stage || !image || zoom <= SCREENSHOT_DEFAULT_ZOOM) {
      return SCREENSHOT_DEFAULT_PAN;
    }
    const naturalWidth = image.naturalWidth || 16;
    const naturalHeight = image.naturalHeight || 9;
    const containScale = Math.min(
      stage.clientWidth / naturalWidth,
      stage.clientHeight / naturalHeight,
    );
    const containedWidth = naturalWidth * containScale;
    const containedHeight = naturalHeight * containScale;
    const scale = zoom / 100;
    return {
      x: Math.max(0, (containedWidth * scale - stage.clientWidth) / 2),
      y: Math.max(0, (containedHeight * scale - stage.clientHeight) / 2),
    };
  }, [zoom]);

  const panImage = useCallback(
    (direction: "up" | "down" | "left" | "right") => {
      if (zoom <= SCREENSHOT_DEFAULT_ZOOM) return;
      const delta = {
        x:
          direction === "left"
            ? PAN_STEP
            : direction === "right"
              ? -PAN_STEP
              : 0,
        y: direction === "up" ? PAN_STEP : direction === "down" ? -PAN_STEP : 0,
      };
      setPan((currentPan) =>
        clampScreenshotPan(
          { x: currentPan.x + delta.x, y: currentPan.y + delta.y },
          getPanBounds(),
        ),
      );
    },
    [getPanBounds, zoom],
  );

  const adjustZoom = useCallback(
    (delta: -1 | 1) => {
      setZoom((currentZoom) => {
        const nextZoom = getZoomAfterStep(currentZoom, delta);
        if (nextZoom === SCREENSHOT_DEFAULT_ZOOM)
          setPan(SCREENSHOT_DEFAULT_PAN);
        return nextZoom;
      });
      markInteraction();
    },
    [markInteraction],
  );

  const toggleControls = useCallback(() => {
    markInteraction();
    setControlsVisible((visible) => !visible);
  }, [markInteraction]);

  const toggleFullscreen = useCallback(() => {
    markInteraction();
    setIsFullscreen((fullscreen) => {
      const nextFullscreen = !fullscreen;
      if (nextFullscreen) setControlsVisible(false);
      return nextFullscreen;
    });
  }, [markInteraction]);

  const handleAction = useCallback(
    (action: NavigationAction): boolean => {
      if (isClosing) return true;
      markInteraction();
      const activeFocusId = engine.getActiveFocusId();
      if (action === "previous-primary-screen") {
        adjustZoom(-1);
        return true;
      }
      if (action === "next-primary-screen") {
        adjustZoom(1);
        return true;
      }
      if (action === "confirm") {
        if (activeFocusId === FULLSCREEN_FOCUS_ID) toggleFullscreen();
        else if (activeFocusId === PREVIOUS_FOCUS_ID) changeScreenshot(-1);
        else if (activeFocusId === NEXT_FOCUS_ID) changeScreenshot(1);
        else toggleControls();
        return true;
      }
      if (action === "move-up" && activeFocusId === IMAGE_FOCUS_ID) {
        if (zoom === SCREENSHOT_DEFAULT_ZOOM) engine.focus(FULLSCREEN_FOCUS_ID);
        else panImage("up");
        return true;
      }
      if (action === "move-down" && activeFocusId === FULLSCREEN_FOCUS_ID) {
        engine.focus(IMAGE_FOCUS_ID);
        return true;
      }
      if (action === "move-left") {
        if (zoom > SCREENSHOT_DEFAULT_ZOOM) panImage("left");
        else changeScreenshot(-1);
        return true;
      }
      if (action === "move-right") {
        if (zoom > SCREENSHOT_DEFAULT_ZOOM) panImage("right");
        else changeScreenshot(1);
        return true;
      }
      if (action === "move-down" || action === "move-up") {
        panImage(action === "move-up" ? "up" : "down");
        return true;
      }
      return false;
    },
    [
      adjustZoom,
      changeScreenshot,
      engine,
      isClosing,
      markInteraction,
      panImage,
      toggleControls,
      toggleFullscreen,
      zoom,
    ],
  );

  const handleClose = useCallback(() => {
    if (isClosing) return;
    clearHideControlsTimer();
    clearIdleTimer();
    setIsClosing(true);
    closeTimerRef.current = window.setTimeout(() => {
      closeTimerRef.current = null;
      engine.requestScopeRestore(
        VIEWER_SCOPE_ID,
        "details",
        `details-screenshot-viewer-close-${currentIndex}`,
      );
      onClose();
    }, CLOSE_DURATION_MS);
  }, [
    clearHideControlsTimer,
    clearIdleTimer,
    currentIndex,
    engine,
    isClosing,
    onClose,
  ]);

  const currentScreenshot = screenshots[currentIndex];
  if (!currentScreenshot) return null;

  const displayedRect = isClosing ? origin : isOpening ? origin : targetRect;
  const sharedElementStyle: CSSProperties | undefined = displayedRect
    ? {
        position: "fixed",
        left: displayedRect.left,
        top: displayedRect.top,
        width: displayedRect.width,
        height: displayedRect.height,
        borderRadius: displayedRect.borderRadius,
        boxShadow: displayedRect.boxShadow,
      }
    : undefined;

  return (
    <div
      className={`details-screenshot-viewer-backdrop${isClosing ? " is-closing" : ""}${isFullscreen ? " is-fullscreen" : ""}`}
    >
      <NavigationDialog
        scopeId={VIEWER_SCOPE_ID}
        initialFocusId={IMAGE_FOCUS_ID}
        className="details-screenshot-viewer"
        onAction={handleAction}
        onBack={() => {
          handleClose();
          return true;
        }}
      >
        <div
          className="details-screenshot-viewer-viewport"
          aria-label={`${gameTitle} screenshot viewer`}
        >
          <div className="details-screenshot-viewer-stage-shell">
            <div ref={stageRef} className="details-screenshot-viewer-stage">
              <div
                className={`details-screenshot-viewer-counter${controlsVisible ? " is-visible" : ""}`}
                aria-hidden={!controlsVisible}
              >
                {currentIndex + 1} / {screenshots.length}
              </div>
              <Focusable
                focusId={FULLSCREEN_FOCUS_ID}
                scopeId={VIEWER_SCOPE_ID}
                className={`details-screenshot-viewer-fullscreen${controlsVisible ? " is-visible" : ""}`}
                ariaLabel={
                  isFullscreen ? "Exit fullscreen" : "Enter fullscreen"
                }
                ariaPressed={isFullscreen}
                disabled={!controlsVisible}
                onConfirm={toggleFullscreen}
              >
                <span aria-hidden="true">⛶</span>
              </Focusable>
              <Focusable
                focusId={PREVIOUS_FOCUS_ID}
                scopeId={VIEWER_SCOPE_ID}
                className={`details-screenshot-viewer-side-arrow previous${controlsVisible ? " is-visible" : ""}${isIdle ? " is-idle" : ""}`}
                ariaLabel="Previous screenshot"
                disabled={!controlsVisible}
                onConfirm={() => changeScreenshot(-1)}
              >
                <span aria-hidden="true">‹</span>
              </Focusable>
              <Focusable
                focusId={IMAGE_FOCUS_ID}
                scopeId={VIEWER_SCOPE_ID}
                className="details-screenshot-viewer-image-button"
                ariaLabel={`${gameTitle} screenshot ${currentIndex + 1}`}
                style={sharedElementStyle}
                onConfirm={toggleControls}
              >
                <MediaImage
                  imageRef={imageRef}
                  gameId={gameId}
                  mediaType="screenshot"
                  key={`${currentScreenshot}-${currentIndex}`}
                  src={currentScreenshot}
                  alt={`${gameTitle} screenshot ${currentIndex + 1}`}
                  className="details-screenshot-viewer-image"
                  draggable={false}
                  style={{
                    transform: `translate3d(${pan.x}px, ${pan.y}px, 0) scale(${zoom / 100})`,
                  }}
                  onReady={() => setImageLoadVersion((version) => version + 1)}
                />
              </Focusable>
              <Focusable
                focusId={NEXT_FOCUS_ID}
                scopeId={VIEWER_SCOPE_ID}
                className={`details-screenshot-viewer-side-arrow next${controlsVisible ? " is-visible" : ""}${isIdle ? " is-idle" : ""}`}
                ariaLabel="Next screenshot"
                disabled={!controlsVisible}
                onConfirm={() => changeScreenshot(1)}
              >
                <span aria-hidden="true">›</span>
              </Focusable>
            </div>
          </div>
          <div
            className={`details-screenshot-viewer-thumbnails${controlsVisible ? " is-visible" : ""}`}
            aria-hidden="true"
          >
            {screenshots.map((screenshot, index) => (
              <span
                className={`details-screenshot-viewer-thumbnail${index === currentIndex ? " is-selected" : ""}`}
                key={`${screenshot}-${index}`}
              >
                <MediaImage
                  gameId={gameId}
                  mediaType="screenshot"
                  src={screenshot}
                  alt=""
                  draggable={false}
                />
                {index === currentIndex ? (
                  <span
                    className="details-screenshot-viewer-thumbnail-check"
                    aria-hidden="true"
                  >
                    &#x2713;
                  </span>
                ) : null}
              </span>
            ))}
          </div>
        </div>
        <div
          className={`details-screenshot-viewer-hud${controlsVisible ? " is-visible" : ""}`}
          aria-hidden={!controlsVisible}
        >
          <span className="details-screenshot-viewer-controls">
            <span className="details-screenshot-viewer-hud-item">
              <span
                className="details-screenshot-viewer-hud-icon"
                aria-hidden="true"
              >
                &#x24D8;
              </span>
              <span>Mostrar / Ocultar controles</span>
            </span>
            <span className="details-screenshot-viewer-hud-item">
              <span
                className="details-screenshot-viewer-hud-keys"
                aria-hidden="true"
              >
                <b>LB</b>
                <b>RB</b>
              </span>
              <span>Cambiar imagen</span>
            </span>
            <span className="details-screenshot-viewer-hud-item">
              <span
                className="details-screenshot-viewer-hud-keys"
                aria-hidden="true"
              >
                <b>LT</b>
                <b>RT</b>
              </span>
              <span>Zoom</span>
            </span>
            <span className="details-screenshot-viewer-hud-item">
              <span
                className="details-screenshot-viewer-hud-icon"
                aria-hidden="true"
              >
                &#x2733;
              </span>
              <span>Mover</span>
            </span>
            <span className="details-screenshot-viewer-hud-item is-close">
              <span
                className="details-screenshot-viewer-hud-keys"
                aria-hidden="true"
              >
                <b>B</b>
              </span>
              <span>Cerrar</span>
            </span>
            <span>← → Cambiar imagen</span>
          </span>
        </div>
      </NavigationDialog>
    </div>
  );
}
