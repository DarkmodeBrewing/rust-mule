import { defineConfig } from "vitepress";

export default defineConfig({
  title: "rust-mule",
  description: "Documentation for rust-mule",
  cleanUrls: true,
  themeConfig: {
    nav: [
      { text: "Home", link: "/" },
      { text: "Architecture", link: "/architecture" },
      { text: "API", link: "/API_DESIGN" },
      { text: "Tasks", link: "/TASKS" },
    ],
    sidebar: [
      { text: "Overview", items: [{ text: "Index", link: "/" }] },
      {
        text: "Core Docs",
        items: [
          { text: "Development", link: "/dev" },
          { text: "Architecture", link: "/architecture" },
          { text: "API Design", link: "/API_DESIGN" },
          { text: "UI Design", link: "/UI_DESIGN" },
          { text: "API Curl", link: "/api_curl" },
        ],
      },
      {
        text: "Planning",
        items: [
          { text: "Tasks", link: "/TASKS" },
          { text: "TODO", link: "/TODO" },
          { text: "KAD Parity", link: "/kad_parity" },
          { text: "UI API Contract Map", link: "/ui_api_contract_map" },
          { text: "Handoff", link: "/handoff" },
        ],
      },
    ],
    socialLinks: [
      { icon: "github", link: "https://github.com/DarkmodeBrewing/rust-mule" },
    ],
  },
});
