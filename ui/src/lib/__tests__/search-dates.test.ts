import { describe, it, expect } from "vitest";
import { searchWindow, startOfLocalDay } from "../search-dates";

describe("startOfLocalDay", () => {
  it("is local midnight of that day", () => {
    const instant = new Date(startOfLocalDay("2026-09-15")!);
    expect([instant.getFullYear(), instant.getMonth(), instant.getDate()]).toEqual([2026, 8, 15]);
    expect([instant.getHours(), instant.getMinutes()]).toEqual([0, 0]);
  });

  it("rolls over the end of a month", () => {
    const instant = new Date(startOfLocalDay("2026-09-30", 1)!);
    expect([instant.getMonth(), instant.getDate()]).toEqual([9, 1]);
  });

  it("refuses anything but a calendar day", () => {
    expect(startOfLocalDay("")).toBeUndefined();
    expect(startOfLocalDay("15/09/2026")).toBeUndefined();
  });
});

describe("searchWindow", () => {
  it("includes the whole of the last day", () => {
    const { after, before } = searchWindow("2026-09-15", "2026-09-15");
    expect(new Date(before!).getTime() - new Date(after!).getTime()).toBe(
      new Date(2026, 8, 16).getTime() - new Date(2026, 8, 15).getTime(),
    );
    expect(new Date(before!).getDate()).toBe(16);
  });

  it("leaves an unpicked bound open", () => {
    expect(searchWindow("", "")).toEqual({ after: undefined, before: undefined });
    expect(searchWindow("2026-09-15", "").before).toBeUndefined();
    expect(searchWindow("", "2026-09-15").after).toBeUndefined();
  });
});
