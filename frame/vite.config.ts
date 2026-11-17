import { defineConfig } from "vite-plus";
import { frameConfig } from "./vite.frame.ts";

export default defineConfig(frameConfig(["browser", "column"]));
