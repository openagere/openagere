#!/usr/bin/env node

const pkg = require('../package.json');

const commands = {
  '--version': () => {
    console.log(`openagere v${pkg.version}`);
  },
  '--help': () => {
    console.log(`
OpenAgere - Multi-agent framework (demo placeholder)

Usage:
  openagere [command] [options]

Commands:
  hello        Show a welcome message
  --version    Show version number
  --help       Show help

This is a demo placeholder package. Full version coming soon.
`);
  },
  hello: () => {
    console.log(`
╔══════════════════════════════════════════╗
║                                          ║
║   Welcome to OpenAgere v${pkg.version}           ║
║                                          ║
║   Multi-agent framework for building     ║
║   intelligent, collaborative AI agents.  ║
║                                          ║
║   🚧  This is a demo placeholder.        ║
║   Full version coming soon.              ║
║                                          ║
║   GitHub: github.com/openagere/openagere ║
║                                          ║
╚══════════════════════════════════════════╝
`);
  }
};

function main() {
  const args = process.argv.slice(2);
  const command = args[0];

  if (!command || command === '--version') {
    commands['--version']();
    return;
  }

  if (commands[command]) {
    commands[command]();
    return;
  }

  console.error(`Unknown command: ${command}`);
  console.error('Run "openagere --help" for usage information.');
  process.exit(1);
}

main();
