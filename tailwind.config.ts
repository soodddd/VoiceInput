import type { Config } from "tailwindcss";

/**
 * Tailwind CSS 配置
 * 扫描 src/ 下所有 HTML/JS/TS/JSX/TSX 文件，按需生成原子化样式。
 */
const config: Config = {
  content: ["./src/**/*.{html,js,ts,jsx,tsx}"],
  theme: {
    extend: {},
  },
  plugins: [],
};

export default config;
