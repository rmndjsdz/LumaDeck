import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ImgHTMLAttributes,
  type Ref,
} from "react";
import {
  mediaManager as sharedMediaManager,
  type MediaManager,
} from "./media-manager";
import { recordMediaTiming, type MediaType } from "./media-timing";

interface MediaImageProps extends ImgHTMLAttributes<HTMLImageElement> {
  gameId: string;
  mediaType: MediaType;
  reactKey?: string;
  manager?: MediaManager;
  onReady?: () => void;
  imageRef?: Ref<HTMLImageElement>;
  canonicalStyle?: CSSProperties;
  canonicalClassName?: string;
}

let nextMediaImageInstanceId = 0;

/**
 * React owns the consumer element and all of its layout. MediaManager only
 * warms and deduplicates the resource in its detached preload image.
 */
export function MediaImage({
  gameId,
  mediaType,
  reactKey,
  manager,
  onReady,
  imageRef,
  canonicalStyle,
  canonicalClassName,
  src,
  alt,
  className,
  style,
  id,
  role,
  title,
  ["aria-hidden"]: ariaHidden,
  ["aria-label"]: ariaLabel,
  draggable,
  loading,
  decoding,
  onLoad,
  onError,
  ...imageProps
}: MediaImageProps) {
  const instanceIdRef = useRef<number | undefined>(undefined);
  if (instanceIdRef.current === undefined) {
    instanceIdRef.current = ++nextMediaImageInstanceId;
  }
  const instanceId = instanceIdRef.current;
  const imageElementRef = useRef<HTMLImageElement | null>(null);
  const onReadyRef = useRef(onReady);
  onReadyRef.current = onReady;
  const notifiedReadyUrlRef = useRef<string | null>(null);
  const [version, setVersion] = useState(0);
  const visualManager = manager ?? sharedMediaManager;
  const url = src ?? "";
  const snapshot = visualManager.getSnapshot(url);
  const loadingMode = loading ?? "eager";
  const imageClassName = canonicalClassName ?? className;
  const imageStyle: CSSProperties | undefined = canonicalStyle
    ? { ...style, ...canonicalStyle }
    : style;
  const lifecycleDetail = JSON.stringify({
    instanceId,
    reactKey: reactKey ?? null,
    loading: loadingMode,
    decoding: decoding ?? "async",
  });

  const setImageRef = useCallback(
    (image: HTMLImageElement | null) => {
      imageElementRef.current = image;
      if (typeof imageRef === "function") imageRef(image);
      else if (imageRef) imageRef.current = image;
    },
    [imageRef],
  );

  const notifyReady = useCallback(
    (image: HTMLImageElement): void => {
      if (notifiedReadyUrlRef.current === url) return;
      notifiedReadyUrlRef.current = url;
      onReadyRef.current?.();
      recordMediaTiming("MEDIA_IMAGE_ELEMENT_ATTACHED", {
        gameId,
        type: mediaType,
        path: url,
        detail: JSON.stringify({
          instanceId,
          reactKey: reactKey ?? null,
          owner: "react",
          complete: image.complete,
          naturalWidth: image.naturalWidth,
          canonicalKey: url,
        }),
      });
    },
    [gameId, instanceId, mediaType, reactKey, url],
  );

  useEffect(() => {
    if (!url) return;
    void visualManager
      .ensure({ gameId, mediaType, url })
      .catch(() => undefined);
    return visualManager.subscribe(url, () => setVersion((value) => value + 1));
  }, [gameId, mediaType, url, visualManager]);

  useLayoutEffect(() => {
    const image = imageElementRef.current;
    if (
      !image ||
      !url ||
      !image.complete ||
      (image.naturalWidth === 0 && !url.startsWith("data:"))
    ) {
      return;
    }
    notifyReady(image);
  }, [notifyReady, snapshot.state, snapshot.version, url, version]);

  useEffect(() => {
    recordMediaTiming("MEDIA_IMAGE_MOUNT", {
      gameId,
      type: mediaType,
      path: url,
      detail: lifecycleDetail,
    });
    return () => {
      recordMediaTiming("MEDIA_IMAGE_UNMOUNT", {
        gameId,
        type: mediaType,
        path: url,
        detail: lifecycleDetail,
      });
    };
  }, [gameId, lifecycleDetail, mediaType, url]);

  return (
    <img
      {...imageProps}
      ref={setImageRef}
      src={url || undefined}
      alt={alt}
      className={imageClassName}
      style={imageStyle}
      id={id}
      role={role}
      title={title}
      aria-hidden={ariaHidden}
      aria-label={ariaLabel}
      draggable={draggable}
      loading={loadingMode}
      decoding={decoding ?? "async"}
      data-media-state={snapshot.state}
      data-media-key={url}
      data-media-consumer={reactKey ?? String(instanceId)}
      onLoad={(event) => {
        notifyReady(event.currentTarget);
        onLoad?.(event);
      }}
      onError={onError}
    />
  );
}
