import * as net from "net";

export interface ParsedTcpAddress {
  host: string;
  port: number;
}

const HOSTNAME_REGEX =
  /^(?=.{1,253}$)(?:[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*$/;

function parsePort(portStr: string): number | null {
  if (!/^\d+$/.test(portStr)) {
    return null;
  }

  const port = Number(portStr);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    return null;
  }

  return port;
}

function isValidHost(host: string): boolean {
  if (!host || host.trim() !== host || /\s/.test(host)) {
    return false;
  }

  if (net.isIP(host) === 4) {
    return true;
  }

  return HOSTNAME_REGEX.test(host);
}

export function parseTcpAddress(address: string): ParsedTcpAddress | null {
  const trimmed = address.trim();
  if (!trimmed) {
    return null;
  }

  if (trimmed.startsWith("[")) {
    const closeIdx = trimmed.indexOf("]");
    if (closeIdx <= 1) {
      return null;
    }

    const host = trimmed.slice(1, closeIdx);
    const remainder = trimmed.slice(closeIdx + 1);
    if (!remainder.startsWith(":")) {
      return null;
    }

    if (net.isIP(host) !== 6) {
      return null;
    }

    const port = parsePort(remainder.slice(1));
    if (port === null) {
      return null;
    }

    return { host, port };
  }

  const firstColon = trimmed.indexOf(":");
  const lastColon = trimmed.lastIndexOf(":");
  if (firstColon <= 0 || firstColon !== lastColon) {
    return null;
  }

  const host = trimmed.slice(0, firstColon);
  if (!isValidHost(host)) {
    return null;
  }

  const port = parsePort(trimmed.slice(firstColon + 1));
  if (port === null) {
    return null;
  }

  return { host, port };
}

export function isTcpAddress(address: string): boolean {
  return parseTcpAddress(address) !== null;
}

export function extractTcpPort(address: string): number | null {
  const parsed = parseTcpAddress(address);
  return parsed ? parsed.port : null;
}
