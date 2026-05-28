import { describe, it, expect } from "vitest";
import { IPCError } from "./ipc";
import type { AppError } from "./bindings";

describe("IPCError", () => {
  it("carries the backend code and message", () => {
    const err: AppError = { code: "not_found", message: "no such file" };
    const e = new IPCError(err);
    expect(e).toBeInstanceOf(Error);
    expect(e.name).toBe("IPCError");
    expect(e.code).toBe("not_found");
    expect(e.message).toBe("no such file");
  });
});
