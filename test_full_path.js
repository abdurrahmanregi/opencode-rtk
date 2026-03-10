// Simple test to verify daemon can spawn with full path
const { spawn } = require('child_process');

const BINARY_PATH = 'C:\\Users\\abdur\\OneDrive\\Work\\opencode-rtk\\target\\release\\opencode-rtk.exe';

console.log('Attempting to spawn daemon from:', BINARY_PATH);

const child = spawn(BINARY_PATH, [], {
  detached: true,
  stdio: 'ignore',
  windowsHide: true,
});

child.on('spawn', () => {
  console.log('Daemon spawned successfully, PID:', child.pid);
  console.log('Waiting 2 seconds for daemon to start...');
  
  setTimeout(() => {
    console.log('Checking process list...');
    const { execSync } = require('child_process');
    try {
      const output = execSync('tasklist', { encoding: 'utf8' });
      const hasDaemon = output.includes('opencode-rtk.exe');
      console.log('Daemon in process list:', hasDaemon);
      
      if (hasDaemon) {
        console.log('Cleaning up: killing daemon...');
        execSync('taskkill /F /IM opencode-rtk.exe', { windowsHide: true });
        console.log('Daemon killed');
      }
    } catch (e) {
      console.error('Error:', e.message);
    }
    process.exit(0);
  }, 2000);
});

child.on('error', (err) => {
  console.error('Failed to spawn daemon:', err);
  process.exit(1);
});
