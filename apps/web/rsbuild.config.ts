import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';
import { tanstackStart } from '@tanstack/react-start/plugin/rsbuild';

export default defineConfig({
  // IPv4 loopback so VS Code port forwarding reaches it.
  server: { host: '127.0.0.1', port: 3000 },
  plugins: [pluginReact(), tanstackStart()],
});
