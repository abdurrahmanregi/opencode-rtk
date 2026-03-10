// Test actual plugin spawn functionality
const { RTKDaemonClient } = require('./plugin/dist/client');
const { isDaemonRunning, autoStartDaemon, spawnDaemon } = require('./plugin/dist/spawn');
const { execSync } = require('child_process');

const BINARY_PATH = 'C:\\Users\\abdur\\OneDrive\\Work\\opencode-rtk\\target\\release\\opencode-rtk.exe';

async function runTests() {
  console.log('=== Test 1: Check if daemon running ===');
  const client = new RTKDaemonClient('127.0.0.1:9876');
  const running = await isDaemonRunning(client);
  console.log('Daemon running:', running ? '✓ Yes' : '✗ No');
  
  // Kill if running
  if (running) {
    console.log('\n=== Test 2: Kill existing daemon ===');
    try {
      execSync('taskkill /F /IM opencode-rtk.exe', { windowsHide: true });
      console.log('✓ Daemon killed');
    } catch (e) {
      console.log('✗ Could not kill daemon');
    }
    
    await new Promise(r => setTimeout(r, 1000));
  }
  
  // Test spawn
  console.log('\n=== Test 3: Spawn daemon ===');
  const result = spawnDaemon(BINARY_PATH);
  console.log('Spawn success:', result.success ? '✓ Yes' : '✗ No');
  console.log('Process:', result.process ? `PID ${result.process.pid}` : 'null');
  if (result.error) {
    console.log('Error:', result.error);
  }
  
  await new Promise(r => setTimeout(r, 2000));
  
  // Test auto-start
  console.log('\n=== Test 4: Auto-start daemon ===');
  const started = await autoStartDaemon(BINARY_PATH, client);
  console.log('Auto-start success:', started ? '✓ Yes' : '✗ No');
  
  // Verify health
  console.log('\n=== Test 5: Verify health ===');
  const healthy = await isDaemonRunning(client);
  console.log('Health check:', healthy ? '✓ Healthy' : '✗ Not healthy');
  
  // Cleanup
  if (healthy) {
    console.log('\n=== Cleanup ===');
    execSync('taskkill /F /IM opencode-rtk.exe', { windowsHide: true });
    console.log('✓ Daemon killed');
  }
}

runTests().catch(console.error);
