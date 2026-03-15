/**
 * Tests for RTKDaemonClient
 *
 * Note: These are basic tests for daemon reliability features.
 * Integration tests would require running an actual daemon.
 */

import { test, describe, expect } from "bun:test";
import { RTKDaemonClient } from "../client";

// Mock socket for testing
class MockSocket {
  destroyed = false;
  writable = true;
  readable = true;
  readyState: "open" | "closed" | "opening" = "open";

  destroy() {
    this.destroyed = true;
    this.writable = false;
    this.readable = false;
    this.readyState = "closed";
  }

  // Mock write method
  write(_data: string): boolean {
    return this.writable;
  }

  // Mock event methods
  on(_event: string, _handler: (...args: unknown[]) => void): void {
    // Mock implementation
  }

  once(_event: string, _handler: (...args: unknown[]) => void): void {
    // Mock implementation
  }

  off(_event: string, _handler: (...args: unknown[]) => void): void {
    // Mock implementation
  }
}

describe("RTKDaemonClient", () => {
  describe("probeConnection", () => {
    test("returns false when connection is null", async () => {
      const client = new RTKDaemonClient("/tmp/test.sock");
      // @ts-expect-error - accessing private property for testing
      const result = await client.probeConnection();
      expect(result).toBe(false);
    });

    test("returns false when connection is destroyed", async () => {
      const client = new RTKDaemonClient("/tmp/test.sock");
      const mockSocket = new MockSocket();
      mockSocket.destroy();

      // @ts-expect-error - setting private property for testing
      client.connection = mockSocket;

      // @ts-expect-error - accessing private method for testing
      const result = await client.probeConnection();
      expect(result).toBe(false);
    });

    test("returns true when connection is healthy", async () => {
      const client = new RTKDaemonClient("/tmp/test.sock");
      const mockSocket = new MockSocket();

      // @ts-expect-error - setting private property for testing
      client.connection = mockSocket;

      // @ts-expect-error - accessing private method for testing
      const result = await client.probeConnection();
      expect(result).toBe(true);
    });

    test("returns false when socket is not writable", async () => {
      const client = new RTKDaemonClient("/tmp/test.sock");
      const mockSocket = new MockSocket();
      mockSocket.writable = false;

      // @ts-expect-error - setting private property for testing
      client.connection = mockSocket;

      // @ts-expect-error - accessing private method for testing
      const result = await client.probeConnection();
      expect(result).toBe(false);
    });

    test("returns false when socket is not readable", async () => {
      const client = new RTKDaemonClient("/tmp/test.sock");
      const mockSocket = new MockSocket();
      mockSocket.readable = false;

      // @ts-expect-error - setting private property for testing
      client.connection = mockSocket;

      // @ts-expect-error - accessing private method for testing
      const result = await client.probeConnection();
      expect(result).toBe(false);
    });

    test("returns false when socket is not in open state", async () => {
      const client = new RTKDaemonClient("/tmp/test.sock");
      const mockSocket = new MockSocket();
      mockSocket.readyState = "closed";

      // @ts-expect-error - setting private property for testing
      client.connection = mockSocket;

      // @ts-expect-error - accessing private method for testing
      const result = await client.probeConnection();
      expect(result).toBe(false);
    });

    test("handles timeout gracefully", async () => {
      const client = new RTKDaemonClient("/tmp/test.sock");
      const mockSocket = new MockSocket();

      // Make the socket throw an error when accessing properties
      Object.defineProperty(mockSocket, "writable", {
        get() {
          throw new Error("Socket error");
        },
      });

      // @ts-expect-error - setting private property for testing
      client.connection = mockSocket;

      // @ts-expect-error - accessing private method for testing
      const result = await client.probeConnection();
      expect(result).toBe(false);
    });
  });

  describe("disconnect", () => {
    test("destroys connection when called", () => {
      const client = new RTKDaemonClient("/tmp/test.sock");
      const mockSocket = new MockSocket();

      // @ts-expect-error - setting private property for testing
      client.connection = mockSocket;

      client.disconnect();

      // @ts-expect-error - accessing private property for testing
      expect(client.connection).toBeNull();
      expect(mockSocket.destroyed).toBe(true);
    });

    test("handles null connection gracefully", () => {
      const client = new RTKDaemonClient("/tmp/test.sock");

      // Should not throw when connection is null
      expect(() => client.disconnect()).not.toThrow();
    });
  });

  describe("connection reuse", () => {
    test("does not attempt to reconnect when existing connection is healthy", async () => {
      const client = new RTKDaemonClient("/tmp/test.sock");
      const mockSocket = new MockSocket();

      // @ts-expect-error - setting private property for testing
      client.connection = mockSocket;

      // Note: This test verifies that connect() handles healthy connections
      // The actual reconnection logic is tested via integration tests
      // @ts-expect-error - accessing private method for testing
      const result = await client.connect();

      // Should return a socket that is not null and is the same as our mock
      expect(result).not.toBeNull();
      // @ts-expect-error - accessing private property for testing
      expect(client.connection).not.toBeNull();
    });

    test("reconnects when existing connection is destroyed", () => {
      const client = new RTKDaemonClient("/tmp/test.sock");
      const mockSocket = new MockSocket();
      mockSocket.destroy();

      // @ts-expect-error - setting private property for testing
      client.connection = mockSocket;

      // Note: This will try to reconnect and fail (no actual daemon)
      // The important thing is that it attempts reconnection
      // @ts-expect-error - accessing private method for testing
      expect(client.connect()).rejects.toThrow();
    });
  });

  describe("error handling", () => {
    test("handles health check failures gracefully", async () => {
      const client = new RTKDaemonClient("/tmp/test.sock");

      // health() should return false on error
      const result = await client.health();
      expect(result).toBe(false);
    });

    test("handles compress failures gracefully", () => {
      const client = new RTKDaemonClient("/tmp/test.sock");

      // compress() should throw on error (no daemon running)
      expect(
        client.compress({
          command: "test",
          output: "test output",
        })
      ).rejects.toThrow();
    });
  });
});
