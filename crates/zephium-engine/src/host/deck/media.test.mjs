import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { runInNewContext } from "node:vm";

const read = (name) => readFileSync(new URL(name, import.meta.url), "utf8");

function fixture() {
  class Element {
    constructor(tag, attributes = {}) {
      this.tag = tag;
      this.attributes = new Map(Object.entries(attributes));
      this.children = [];
      this.style = { setProperty() {}, removeProperty() {} };
      this.events = new Map();
    }
    appendChild(node) {
      node.parentElement = this;
      this.children.push(node);
      return node;
    }
    remove() {
      this.parentElement.children = this.parentElement.children.filter((node) => node !== this);
      this.parentElement = null;
    }
    matches(selector) {
      if (selector === "img") return this.tag === "img";
      if (selector === "video") return this.tag === "video";
      if (selector === 'div[role="link"]')
        return this.tag === "div" && this.getAttribute("role") === "link";
      if (selector === 'a[href*="/status/"]')
        return this.tag === "a" && this.href.includes("/status/");
      return selector === `[data-testid="${this.getAttribute("data-testid")}"]`;
    }
    closest(selector) {
      return this.matches(selector) ? this : this.parentElement?.closest(selector);
    }
    querySelectorAll(selector) {
      return this.children.flatMap((node) => [
        ...(selector.split(", ").some((part) => node.matches(part)) ? [node] : []),
        ...node.querySelectorAll(selector),
      ]);
    }
    querySelector(selector) {
      return this.querySelectorAll(selector)[0];
    }
    getAttribute(name) {
      return this.attributes.get(name) ?? null;
    }
    setAttribute(name, value) {
      this.attributes.set(name, value);
    }
    removeAttribute(name) {
      this.attributes.delete(name);
    }
    toggleAttribute(name, present) {
      if (present) this.setAttribute(name, "");
      else this.removeAttribute(name);
    }
    addEventListener(name, listener) {
      this.events.set(name, listener);
    }
    removeEventListener(name) {
      this.events.delete(name);
    }
    showPopover() {
      this.open = true;
    }
    hidePopover() {
      this.open = false;
    }
    getBoundingClientRect() {
      return { top: 0 };
    }
  }

  const events = new Map();
  const messages = [];
  const body = new Element("body");
  const tweet = body.appendChild(new Element("article", { "data-testid": "tweet" }));
  const quote = tweet.appendChild(new Element("div", { role: "link" }));
  function photo(parent, post, number, loaded) {
    const anchor = parent.appendChild(new Element("a"));
    anchor.href = `https://x.com/user/status/${post}/photo/${number}`;
    const node = anchor.appendChild(new Element("div", { "data-testid": "tweetPhoto" }));
    const src = `https://pbs.twimg.com/media/photo-${post}-${number}?name=small`;
    const image = node.appendChild(new Element("img", { src }));
    image.alt = "";
    image.currentSrc = loaded ? src : "";
    return node;
  }
  const main = photo(tweet, 1, 1, true);
  const first = photo(quote, 2, 1, false);
  const second = photo(quote, 2, 2, true);
  const window = {
    ipc: { postMessage: (value) => messages.push(value) },
    addEventListener: (name, listener) => events.set(name, listener),
    removeEventListener: (name) => events.delete(name),
  };
  window.top = window;
  runInNewContext(read("./media.js"), {
    window,
    document: {
      body,
      baseURI: "https://x.com/",
      documentElement: new Element("html"),
      createElement: (tag) => new Element(tag),
    },
    location: { hostname: "x.com" },
    HTMLElement: Element,
    createMediaButton: () => new Element("button"),
    createMediaGallery: runInNewContext(read("./media-gallery.js"), {
      URL,
      document: { body },
      createMediaButton: () => new Element("button"),
    }),
    createMediaActions: () => ({ show() {}, hide() {} }),
    URL,
    AbortController,
    innerWidth: 600,
    innerHeight: 400,
    scrollX: 0,
    scrollY: 0,
    scrollTo() {},
    requestAnimationFrame() {},
  });
  return {
    first,
    main,
    second,
    tweet,
    messages,
    click(node) {
      let intercepted = false;
      events.get("click")({
        isTrusted: true,
        button: 0,
        detail: 1,
        composedPath: () => [node],
        preventDefault() {
          intercepted = true;
        },
        stopImmediatePropagation() {},
      });
      return intercepted;
    },
    display() {
      return body.children
        .filter((node) => node.open && node.attributes.has("data-bdeck-media-content"))
        .flatMap((node) => node.children.map(({ src }) => src));
    },
    next() {
      body.children
        .find((node) => node.getAttribute("data-bdeck-media-direction") === "1")
        .events.get("click")();
    },
    close() {
      body.children
        .find((node) => node.attributes.has("data-bdeck-media-close"))
        .events.get("click")();
    },
  };
}

test("a single click opens a photo while its image request is still starting", () => {
  const page = fixture();
  assert.equal(page.click(page.first), true);
  assert.deepEqual(page.messages, ["media.open"]);
  assert.deepEqual(page.display(), ["https://pbs.twimg.com/media/photo-2-1?name=orig"]);
});

test("rapid activation and quote navigation keep one photo visible after the original post leaves", () => {
  const page = fixture();
  page.click(page.first);
  assert.equal(page.click(page.second), true);
  page.tweet.remove();
  page.next();
  page.next();
  assert.deepEqual(page.display(), ["https://pbs.twimg.com/media/photo-2-2?name=orig"]);
  page.close();
  assert.deepEqual(page.display(), []);
  assert.deepEqual(page.messages, ["media.open", "media.close"]);
});

test("reopening starts with only the newly selected post’s media", () => {
  const page = fixture();
  page.click(page.first);
  page.close();
  page.click(page.main);
  assert.deepEqual(page.display(), ["https://pbs.twimg.com/media/photo-1-1?name=orig"]);
});
