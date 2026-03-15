import { describe, expect, test } from "bun:test";
import { extractTcpPort, isTcpAddress, parseTcpAddress } from "../address";

describe("address parsing", () => {
  test("parses IPv4 host and port", () => {
    expect(parseTcpAddress("127.0.0.1:9876")).toEqual({
      host: "127.0.0.1",
      port: 9876,
    });
    expect(isTcpAddress("127.0.0.1:9876")).toBe(true);
    expect(extractTcpPort("127.0.0.1:9876")).toBe(9876);
  });

  test("accepts min and max valid ports", () => {
    expect(parseTcpAddress("localhost:1")).toEqual({
      host: "localhost",
      port: 1,
    });
    expect(parseTcpAddress("localhost:65535")).toEqual({
      host: "localhost",
      port: 65535,
    });
  });

  test("parses hostname and port", () => {
    expect(parseTcpAddress("localhost:8080")).toEqual({
      host: "localhost",
      port: 8080,
    });
    expect(isTcpAddress("localhost:8080")).toBe(true);
    expect(extractTcpPort("localhost:8080")).toBe(8080);
  });

  test("parses bracketed IPv6 and port", () => {
    expect(parseTcpAddress("[::1]:9876")).toEqual({
      host: "::1",
      port: 9876,
    });
    expect(isTcpAddress("[::1]:9876")).toBe(true);
    expect(extractTcpPort("[::1]:9876")).toBe(9876);
  });

  test("does not classify unix socket path as TCP", () => {
    expect(parseTcpAddress("/tmp/opencode-rtk.sock")).toBeNull();
    expect(isTcpAddress("/tmp/opencode-rtk.sock")).toBe(false);
    expect(extractTcpPort("/tmp/opencode-rtk.sock")).toBeNull();
  });

  test("rejects malformed addresses", () => {
    const malformed = [
      "127.0.0.1",
      "localhost:abc",
      "[::1]",
      "[not-ipv6]:9876",
      "[::1]x:80",
      "::1:9876",
      "localhost:0",
      "[::1]:99999",
      "[::1",
      ":9876",
      "host:-1",
      "",
    ];

    for (const address of malformed) {
      expect(parseTcpAddress(address)).toBeNull();
      expect(isTcpAddress(address)).toBe(false);
      expect(extractTcpPort(address)).toBeNull();
    }
  });
});
