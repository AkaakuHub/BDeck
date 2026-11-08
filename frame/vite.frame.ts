import { fileURLToPath } from "node:url";
import type { UserConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { alias } from "./aliases.ts";

type Page = "browser" | "column";

export function frameConfig(pages: readonly Page[]): UserConfig {
  return {
    appType: "mpa",
    resolve: { alias },
    plugins: [svelte(), tailwindcss()],
    clearScreen: false,
    optimizeDeps: { include: ["svelte/reactivity"] },
    server: { port: 1420, strictPort: true },
    build: {
      emptyOutDir: true,
      target: "es2022",
      modulePreload: false,
      rollupOptions: {
        input: Object.fromEntries(
          pages.map((page) => [page, fileURLToPath(new URL(`./${page}.html`, import.meta.url))]),
        ),
      },
    },
  };
}
