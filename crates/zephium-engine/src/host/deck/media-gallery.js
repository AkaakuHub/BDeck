(photo, imageForPhoto, select) => {
  const tweet = photo?.closest('[data-testid="tweet"]');
  const quoteCard = (node) => node.parentElement?.closest('div[role="link"]');
  const photoPost = (node) => {
    const link = node.closest('a[href*="/status/"]');
    return link
      ? new URL(link.href).pathname.match(/\/status\/(\d+)\/photo\/\d+\/?$/)?.[1]
      : undefined;
  };
  const post = photoPost(photo);
  const quote = quoteCard(photo);
  const images = tweet
    ? [...tweet.querySelectorAll('[data-testid="tweetPhoto"]')]
        .filter(
          (node) =>
            node.closest('[data-testid="tweet"]') === tweet &&
            quoteCard(node) === quote &&
            photoPost(node) === post &&
            !node.querySelector('[data-testid="playButton"], video'),
        )
        .map((node) => ({ selected: node === photo, image: imageForPhoto(node) }))
        .filter(({ image }) => image)
    : [];
  let index = images.findIndex(({ selected }) => selected);
  if (images.length < 2 || index < 0) return null;
  const buttons = [-1, 1].map((direction) => {
    const button =
      direction < 0
        ? createMediaButton("前の画像", "previous")
        : createMediaButton("次の画像", "next");
    button.setAttribute("popover", "manual");
    button.setAttribute("data-bdeck-media-direction", String(direction));
    button.addEventListener("click", () => move(direction));
    document.body.appendChild(button);
    return button;
  });
  const updateButtons = () =>
    buttons.forEach((button, offset) => {
      button.disabled = offset === 0 ? index === 0 : index === images.length - 1;
    });
  const move = (direction) => {
    const next = index + direction;
    if (next < 0 || next >= images.length) return false;
    index = next;
    select(images[index].image);
    updateButtons();
    return true;
  };
  return {
    move,
    show() {
      updateButtons();
      buttons.forEach((button) => button.showPopover());
    },
    dispose: () => buttons.forEach((button) => button.remove()),
  };
};
