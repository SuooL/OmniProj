import { describe, expect, it } from "vitest";

import { applyOverviewToCaches, createQueryClient } from "./queryClient";
import { projectId } from "./domain/project";
import { queryKeys } from "./queryKeys";
import { indexItem, indexResponse, overview, projectSource } from "./test/fixtures";

describe("applyOverviewToCaches", () => {
  it("patches the matching Index row and sets the Overview cache from the returned DTO", () => {
    const client = createQueryClient();
    const a = projectId("project-a");
    const b = projectId("project-b");
    client.setQueryData(
      queryKeys.projectIndex,
      indexResponse([
        indexItem({ project_id: a, name: "A", status: "active", revision: 1, source_revision: 1 }),
        indexItem({ project_id: b, name: "B" }),
      ]),
    );

    const updated = overview({
      project_id: a,
      name: "A2",
      status: "waiting",
      revision: 7,
      source: projectSource({ status: "unreadable", revision: 4 }),
    });
    applyOverviewToCaches(client, updated);

    const index = client.getQueryData(queryKeys.projectIndex) as ReturnType<typeof indexResponse>;
    const rowA = index.projects.find((row) => row.project_id === a)!;
    expect(rowA.name).toBe("A2");
    expect(rowA.status).toBe("waiting");
    expect(rowA.revision).toBe(7);
    expect(rowA.source_status).toBe("unreadable");
    expect(rowA.source_revision).toBe(4);
    // The other row is untouched.
    expect(index.projects.find((row) => row.project_id === b)!.name).toBe("B");
    // The Overview cache is populated.
    expect(client.getQueryData(queryKeys.projectOverview(a))).toEqual(updated);
  });

  it("leaves the Index array reference unchanged when the project is not present", () => {
    const client = createQueryClient();
    const known = projectId("known");
    client.setQueryData(queryKeys.projectIndex, indexResponse([indexItem({ project_id: known })]));
    const before = client.getQueryData(queryKeys.projectIndex);

    applyOverviewToCaches(client, overview({ project_id: projectId("stranger") }));

    expect(client.getQueryData(queryKeys.projectIndex)).toBe(before); // identity preserved
    // ...but the stranger's Overview is still cached.
    expect(client.getQueryData(queryKeys.projectOverview(projectId("stranger")))).not.toBeUndefined();
  });

  it("keeps the row's prior source fields when the Overview has no source", () => {
    const client = createQueryClient();
    const id = projectId("no-source");
    client.setQueryData(
      queryKeys.projectIndex,
      indexResponse([
        indexItem({ project_id: id, source_status: "available", source_revision: 3 }),
      ]),
    );

    applyOverviewToCaches(client, overview({ project_id: id, source: null, revision: 9 }));

    const index = client.getQueryData(queryKeys.projectIndex) as ReturnType<typeof indexResponse>;
    const row = index.projects[0];
    expect(row.revision).toBe(9); // human-state fields update
    expect(row.source_status).toBe("available"); // source fields preserved
    expect(row.source_revision).toBe(3);
  });
});
