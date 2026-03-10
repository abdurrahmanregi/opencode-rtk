// Test script with fixes
const { spawn } = require('child_process');
const path = require('path');

const BINARY_PATH = 'C:\\Users\\abdur\\OneDrive\\Work\\opencode-rtk\\target\\release\\opencode-rtk.exe';

console.log('=== Test 1: Spawn daemon ===');
const child = spawn(BINARY_PATH, [], {
  detached: true,
  stdio: 'ignore',
  windowsHide: true,
});

child.on('spawn', () => {
  console.log('✓ Daemon spawned successfully, PID:', child.pid);
});

child.on('error', (err) => {
  console.error('✗ Spawn error:', err);
  process.exit(1);
});

// Wait and cleanup
setTimeout(() => {
  console.log('\n=== Test 2: Check process list ===');
  const { execSync } = require('child_process');
  try {
    const output = execSync('tasklist', { encoding: 'utf8' });
    const hasDaemon = output.includes('opencode-rtk.exe');
    console.log('Daemon in process list:', hasDaemon ? '✓ Yes' : '✗ No');
    
    if (hasDaemon) {
      console.log('\n=== Test 3: Cleanup - kill daemon ===');
      execSync('taskkill /F /IM opencode-rtk.exe', { windowsHide: true });
      console.log('✓ Daemon killed');
      
      setTimeout(() => {
        const output2 = execSync('tasklist', { encoding: 'utf8' });
        const stillRunning = output2.includes('opencode-rtk.exe');
        console.log('Daemon after kill:', stillRunning ? '✗ Still running (ERROR)' : '✓ Successfully stopped');
        process.exit(0);
      }, 500);
    } else {
      process.exit(1);
    }
  } catch (e) {
    console.error('Error:', e.message);
    process.exit(1);
  }
}, 2000);
