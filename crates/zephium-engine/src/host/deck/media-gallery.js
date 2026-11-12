(element, select) => {
  const photo = element.closest('[data-testid="tweetPhoto"]');
  const tweet = photo?.closest('[data-testid="tweet"]');
  const images = tweet ? [...tweet.querySelectorAll('[data-testid="tweetPhoto"]')]
    .filter(node => node.closest('[data-testid="tweet"]') === tweet && !node.querySelector('[data-testid="playButton"], video'))
    .map(node => node.querySelector('[style*="background-image"]'))
    .filter(node => node instanceof HTMLElement) : [];
  let index = images.indexOf(element);
  if (images.length < 2 || index < 0) return null;
  const buttons = [-1, 1].map(direction => {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = direction < 0 ? "‹" : "›";
    button.setAttribute("aria-label", direction < 0 ? "前の画像" : "次の画像");
    button.setAttribute("popover", "manual");
    button.setAttribute("data-bdeck-media-direction", String(direction));
    button.addEventListener("click", () => move(direction));
    document.body.appendChild(button);
    return button;
  });
  const show = () => buttons.forEach((button, offset) => {
    button.disabled = offset === 0 ? index === 0 : index === images.length - 1;
    button.hidePopover();
    button.showPopover();
  });
  const move = direction => {
    const next = index + direction;
    if (next >= 0 && next < images.length && images[next].isConnected) {
      index = next;
      select(images[index]);
    }
  };
  return { move, show, dispose: () => buttons.forEach(button => button.remove()) };
}
