// Enforces ADR-005 convention: list operations must return { items: [...], cursor? }
// and never a bare JSON array.
//
// Checks two things:
// 1. Any operation with operationId starting with "list" must NOT have a bare
//    array (type: array) as its 200 response schema.
// 2. Any $ref target used for such responses must point to a schema containing
//    an "items" property.

const HTTP_METHODS = new Set([
  "get", "put", "post", "delete", "patch", "options", "head", "trace",
]);

function getSchema() {
  return {
    name: "burst-list-response-format",
    description:
      "List operations must return an object with an 'items' property, not a bare array (ADR-005)",
  };
}

function runRule(input) {
  if (!input || typeof input !== "object") return [];

  const paths = input.paths;
  const schemas = input.components && input.components.schemas;
  if (!paths || typeof paths !== "object") return [];

  const results = [];

  for (const [path, pathItem] of Object.entries(paths)) {
    if (!pathItem || typeof pathItem !== "object") continue;

    for (const [method, operation] of Object.entries(pathItem)) {
      if (!HTTP_METHODS.has(method)) continue;
      if (!operation || typeof operation !== "object") continue;

      const opId = operation.operationId;
      if (!opId || !opId.startsWith("list")) continue;

      const resp200 = operation.responses && operation.responses["200"];
      if (!resp200) continue;

      const content = resp200.content;
      if (!content) continue;

      const jsonContent = content["application/json"];
      if (!jsonContent || !jsonContent.schema) continue;

      const schema = jsonContent.schema;

      // Case 1: inline array schema
      if (schema.type === "array") {
        results.push({
          message: `${method.toUpperCase()} ${path} (${opId}): 200 response must be an object with "items" property, not a bare array (ADR-005).`,
        });
        continue;
      }

      // Case 2: $ref — resolve and check the target schema has "items"
      if (schema["$ref"] && schemas) {
        const refName = schema["$ref"].replace("#/components/schemas/", "");
        const target = schemas[refName];
        if (target && target.properties && !target.properties.items) {
          results.push({
            message: `${method.toUpperCase()} ${path} (${opId}): referenced schema "${refName}" must have an "items" property (ADR-005).`,
          });
        }
        if (target && target.type === "array") {
          results.push({
            message: `${method.toUpperCase()} ${path} (${opId}): referenced schema "${refName}" must be an object, not an array (ADR-005).`,
          });
        }
        continue;
      }

      // Case 3: inline object — check it has "items"
      if (schema.type === "object" && schema.properties && !schema.properties.items) {
        results.push({
          message: `${method.toUpperCase()} ${path} (${opId}): 200 response object must have an "items" property (ADR-005).`,
        });
      }
    }
  }

  return results;
}
