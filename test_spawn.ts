// Test script to verify daemon spawn functionality
import { RTKDaemonClient } from "./plugin/dist/client";
import { isDaemonRunning, spawnDaemon, autoStartDaemon } from "./plugin/dist/spawn";
import * as cp from "child_process";

async function testSpawn() {
  console.log("=== Test 1: Check if daemon is already running ===");
  const client = new RTKDaemonClient("127.0.0.1:9876");
  const isRunning = await isDaemonRunning(client);
  console.log(`Daemon running: ${isRunning}`);
  
  // Kill existing daemon
  if (isRunning) {
    console.log("\n=== Test 2: Killing existing daemon ===");
    try {
      cp.execSync("taskkill /F /IM opencode-rtk.exe", { windowsHide: true });
      console.log("Daemon killed");
    } catch (e) {
      console.log("Could not kill daemon (may not exist):", e);
    }
    
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  
  // Test spawn
  console.log("\n=== Test 3: Spawning daemon ===");
  const child = spawnDaemon("opencode-rtk");
  console.log(`Spawn result: ${child ? "success" : "failed"}`);
  if (child) {
    console.log(`PID: ${child.pid}`);
  }
  
  // Test auto-start
  console.log("\n=== Test 4: Auto-start daemon ===");
  const client2 = new RTKDaemonClient("127.0.0.1:9876");
  const started = await autoStartDaemon("opencode-rtk", client2);
  console.log(`Auto-start result: ${started}`);
  
  // Cleanup
  if (started) {
    console.log("\n=== Cleanup: Killing spawned daemon ===");
    try {
      cp.execSync("taskkill /F /IM opencode-rtk.exe", { windowsHide: true });
      console.log("Daemon killed");
    } catch (e) {
      console.log("Could not kill daemon:", e);
    }
  }
}

testSpawn().catch(console.error);
