(currentMedia) => {
  const actions = document.createElement("div");
  actions.setAttribute("popover", "manual");
  actions.setAttribute("data-bdeck-media-actions", "");

  const status = document.createElement("span");
  status.setAttribute("role", "status");
  let statusFade = null;
  const setStatus = (message, dismiss = false) => {
    statusFade?.cancel();
    statusFade = null;
    status.textContent = message;
    if (!message || !dismiss) return;
    const fade = status.animate([{ opacity: 1 }, { opacity: 0 }], {
      delay: 3000,
      duration: matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 250,
      fill: "forwards",
    });
    statusFade = fade;
    fade.onfinish = () => {
      if (statusFade === fade) setStatus("");
    };
  };

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
  const button = (label, icon, operation, failure, success = "") => {
    const element = createMediaButton(label, icon);
    let pending = null;
    element.addEventListener("click", async (event) => {
      const media = currentMedia();
      if (!event.isTrusted || !media || pending === media) return;
      pending = media;
      setStatus("処理中…");
      try {
        await operation(media);
        if (currentMedia() === media) setStatus(success, true);
      } catch {
        if (currentMedia() === media) setStatus(failure, true);
      } finally {
        if (pending === media) pending = null;
      }
    });
    actions.appendChild(element);
    return element;
  };
  button("ダウンロード", "download", download, "ダウンロードできませんでした");
  const copyButton = button(
    "画像をコピー",
    "copy",
    copy,
    "コピーできませんでした",
    "画像をコピーしました",
  );
  actions.appendChild(status);
  document.body.appendChild(actions);
  return {
    show(video) {
      setStatus("");
      copyButton.hidden = Boolean(video);
      actions.hidePopover();
      actions.showPopover();
    },
    hide() {
      setStatus("");
      actions.hidePopover();
    },
  };
};
