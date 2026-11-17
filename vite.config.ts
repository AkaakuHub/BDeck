import { defineConfig } from "vite-plus";

const commonIgnored = [
  "**/node_modules/**",
  "**/dist/**",
  "**/target/**",
  "**/vendor/**",
  "frame/src/shared/i18n/**",
  "frame/src/shared/ipc/bindings.ts",
  "frame/src/vite-env.d.ts",
  "frame/project.inlang/**",
  "frame/reports/**",
];

const lintIgnored = [...commonIgnored, "crates/**", "desktop/**", "scripts/**", "xtask/**"];

export default defineConfig({
  fmt: {
    ignorePatterns: commonIgnored,
  },
  lint: {
    ignorePatterns: lintIgnored,
    options: {
      typeAware: true,
      typeCheck: true,
    },
    rules: {
      "no-console": ["error", { allow: ["warn", "error"] }],
      "typescript/no-explicit-any": "error",
    },
    overrides: [
      {
        files: ["frame/tests/**/*.mjs"],
        rules: {
          "typescript/no-floating-promises": "off",
        },
      },
    ],
  },
  staged: {
    "*.{js,mjs,cjs,ts,tsx,svelte,json,css,md}": "vp check --fix",
  },
});
