import kirklin from "@kirklin/eslint-config";

export default kirklin({
  react: false,
  typescript: true,
  formatters: true,
  nextjs: false,
},
// --- Custom Rule Overrides ---
{
  ignores: [
    "src-tauri",
    "src-tauri/**",
  ],
}, {
  rules: {
    "node/prefer-global/process": "off", // Allow using `process.env`
  },
});
