import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { MediaImage } from "./MediaImage";
import { MediaManager } from "./media-manager";

describe("MediaImage lifecycle", () => {
  let host: HTMLDivElement | undefined;
  let manager: MediaManager | undefined;

  afterEach(() => {
    manager?.dispose();
    manager = undefined;
    host?.remove();
    host = undefined;
  });

  it("keeps a React-owned image element stable across route-data rerenders", async () => {
    const images: HTMLImageElement[] = [];
    manager = new MediaManager({
      imageFactory: () => {
        const image = document.createElement("img");
        Object.defineProperty(image, "decode", {
          configurable: true,
          value: () => Promise.resolve(),
        });
        images.push(image);
        return image;
      },
    });
    host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);
    const src = "http://127.0.0.1:43210/media?path=game-screenshot.webp";

    await act(async () => {
      root.render(
        <MediaImage
          manager={manager}
          gameId="steam-678950"
          mediaType="screenshot"
          reactKey="steam-678950-screenshot-0"
          src={src}
        />,
      );
    });
    expect(images).toHaveLength(1);

    await act(async () => {
      images[0]?.onload?.(new Event("load"));
      await Promise.resolve();
    });

    const image = host.querySelector("img");
    expect(image).not.toBeNull();
    expect(image).not.toBe(images[0]);
    expect(image?.dataset.mediaKey).toBe(src);

    await act(async () => {
      root.render(
        <MediaImage
          className="updated"
          manager={manager}
          gameId="steam-678950"
          mediaType="screenshot"
          reactKey="steam-678950-screenshot-0"
          src={src}
        />,
      );
    });

    expect(host.querySelector("img")).toBe(image);
    await act(async () => root.unmount());
  });

  it("renders two independently styled consumers of one decoded resource", async () => {
    const images: HTMLImageElement[] = [];
    manager = new MediaManager({
      imageFactory: () => {
        const image = document.createElement("img");
        Object.defineProperty(image, "decode", {
          configurable: true,
          value: () => Promise.resolve(),
        });
        images.push(image);
        return image;
      },
    });
    host = document.createElement("div");
    const secondHost = document.createElement("div");
    document.body.append(host, secondHost);
    const firstRoot = createRoot(host);
    const secondRoot = createRoot(secondHost);
    const src = "http://127.0.0.1:43210/media?path=shared-cover.webp";

    await act(async () => {
      firstRoot.render(
        <MediaImage
          manager={manager}
          gameId="game-shared"
          mediaType="grid"
          reactKey="home-shared"
          src={src}
          style={{ width: "120px", height: "80px", objectFit: "contain" }}
        />,
      );
      secondRoot.render(
        <MediaImage
          manager={manager}
          gameId="game-shared"
          mediaType="grid"
          reactKey="library-shared"
          src={src}
          style={{ width: "240px", height: "160px", objectFit: "cover" }}
        />,
      );
    });
    await act(async () => {
      images[0]?.onload?.(new Event("load"));
      await Promise.resolve();
    });

    const firstImage = host.querySelector("img");
    const secondImage = secondHost.querySelector("img");
    expect(firstImage).not.toBeNull();
    expect(secondImage).not.toBeNull();
    expect(secondImage).not.toBe(firstImage);
    expect(firstImage?.style.width).toBe("120px");
    expect(firstImage?.style.height).toBe("80px");
    expect(firstImage?.style.objectFit).toBe("contain");
    expect(secondImage?.style.width).toBe("240px");
    expect(secondImage?.style.height).toBe("160px");
    expect(secondImage?.style.objectFit).toBe("cover");

    await act(async () => firstRoot.unmount());
    expect(secondHost.querySelector("img")).toBe(secondImage);
    await act(async () => secondRoot.unmount());
    secondHost.remove();
  });
});
