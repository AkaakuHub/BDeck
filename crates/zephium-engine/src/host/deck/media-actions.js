(currentMedia) => {
  const actions = document.createElement("div");
  actions.setAttribute("popover", "manual");
  actions.setAttribute("data-bdeck-media-actions", "");

  const status = document.createElement("span");
  status.setAttribute("role", "status");

  const source = (media) => {
    const url = new URL(media.video ? media.video.currentSrc : media.imageUrl);
    const cdn = media.video ? "video.twimg.com" : "pbs.twimg.com";
    if (
      !(url.protocol === "https:" && url.hostname === cdn) &&
      !(media.video && url.protocol === "blob:" && url.origin === location.origin)
    )
      throw new Error("Unsupported media URL");
    return url;
  };
  const load = async (url, kind) => {
    const response = await fetch(url, { credentials: "omit" });
    if (!response.ok || !response.headers.get("content-type")?.startsWith(kind + "/"))
      throw new Error("Media file unavailable");
    const blob = await response.blob();
    if (!blob.size) throw new Error("Empty media file");
    return blob;
  };
  const download = async (media) => {
    const url = source(media);
    const blob = await load(url, media.video ? "video" : "image");
    if (currentMedia() !== media) return;
    const extension = blob.type.split("/")[1].split(";")[0];
    const filename = url.pathname
      .split("/")
      .pop()
      .replace(/\.[a-z0-9]+$/i, "");
    const objectUrl = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = objectUrl;
    link.download = `${filename}.${extension === "jpeg" ? "jpg" : extension}`;
    actions.appendChild(link);
    try {
      link.click();
    } finally {
      link.remove();
      setTimeout(() => URL.revokeObjectURL(objectUrl), 60_000);
    }
  };
  const png = async (url) => {
    const blob = await load(url, "image");
    if (blob.type === "image/png") return blob;
    const bitmap = await createImageBitmap(blob);
    try {
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      canvas.getContext("2d").drawImage(bitmap, 0, 0);
      return await new Promise((resolve, reject) =>
        canvas.toBlob((image) =>
          image ? resolve(image) : reject(new Error("PNG encoding failed")),
        ),
      );
    } finally {
      bitmap.close();
    }
  };
  const copy = (media) => {
    const clipboard = navigator.clipboard;
    if (!clipboard || typeof ClipboardItem !== "function") throw new Error("Clipboard unavailable");
    const url = source(media);
    const image = png(url);
    image.catch(() => {});
    return clipboard.write([new ClipboardItem({ "image/png": image })]);
  };
  // Download01Icon and Copy01Icon from @hugeicons/core-free-icons.
  const downloadIcon = [
    "M2.99969 17.0002C2.99969 17.9302 2.99969 18.3952 3.10192 18.7767C3.37932 19.8119 4.18796 20.6206 5.22324 20.898C5.60474 21.0002 6.06972 21.0002 6.99969 21.0002L16.9997 21.0002C17.9297 21.0002 18.3947 21.0002 18.7762 20.898C19.8114 20.6206 20.6201 19.8119 20.8975 18.7767C20.9997 18.3952 20.9997 17.9302 20.9997 17.0002",
    "M16.4998 11.5002C16.4998 11.5002 13.1856 16.0002 11.9997 16.0002C10.8139 16.0002 7.49976 11.5002 7.49976 11.5002M11.9997 15.0002V3.00016",
  ];
  const copyIcon = [
    "M7.5 14.5C7.5 11.2002 7.5 9.55025 8.52513 8.52513C9.55025 7.5 11.2002 7.5 14.5 7.5C17.7998 7.5 19.4497 7.5 20.4749 8.52513C21.5 9.55025 21.5 11.2002 21.5 14.5C21.5 17.7998 21.5 19.4497 20.4749 20.4749C19.4497 21.5 17.7998 21.5 14.5 21.5C11.2002 21.5 9.55025 21.5 8.52513 20.4749C7.5 19.4497 7.5 17.7998 7.5 14.5Z",
    "M7.5 16.5C6.10355 16.5 5.40533 16.5 4.84402 16.3036C3.83866 15.9518 3.0482 15.1613 2.69641 14.156C2.5 13.5947 2.5 12.8964 2.5 11.5V9.5C2.5 6.20017 2.5 4.55025 3.52513 3.52513C4.55025 2.5 6.20017 2.5 9.5 2.5H11.5C12.8964 2.5 13.5947 2.5 14.156 2.69641C15.1613 3.0482 15.9518 3.83866 16.3036 4.84402C16.5 5.40533 16.5 6.10355 16.5 7.5",
  ];
  const button = (label, paths, operation, failure, success = "") => {
    const element = document.createElement("button");
    element.type = "button";
    element.setAttribute("aria-label", label);
    element.title = label;
    const namespace = "http://www.w3.org/2000/svg";
    const icon = document.createElementNS(namespace, "svg");
    icon.setAttribute("viewBox", "0 0 24 24");
    icon.setAttribute("aria-hidden", "true");
    icon.setAttribute("fill", "none");
    icon.setAttribute("stroke", "currentColor");
    icon.setAttribute("stroke-linecap", "round");
    icon.setAttribute("stroke-linejoin", "round");
    icon.setAttribute("stroke-width", "1.5");
    for (const data of paths) {
      const path = document.createElementNS(namespace, "path");
      path.setAttribute("d", data);
      icon.appendChild(path);
    }
    element.appendChild(icon);
    let pending = null;
    element.addEventListener("click", async (event) => {
      const media = currentMedia();
      if (!event.isTrusted || !media || pending === media) return;
      pending = media;
      status.textContent = "";
      try {
        await operation(media);
        if (currentMedia() === media) status.textContent = success;
      } catch {
        if (currentMedia() === media) status.textContent = failure;
      } finally {
        if (pending === media) pending = null;
      }
    });
    actions.appendChild(element);
    return element;
  };
  button("ダウンロード", downloadIcon, download, "ダウンロードできませんでした");
  const copyButton = button(
    "画像をコピー",
    copyIcon,
    copy,
    "コピーできませんでした",
    "画像をコピーしました",
  );
  actions.appendChild(status);
  document.body.appendChild(actions);
  return {
    show(video) {
      status.textContent = "";
      copyButton.hidden = Boolean(video);
      actions.hidePopover();
      actions.showPopover();
    },
    hide: () => actions.hidePopover(),
  };
};
