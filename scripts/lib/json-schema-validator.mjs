export function validateSchema(schema, value, path = "$", errors = []) {
  if (schema.const !== undefined && !deepEqual(value, schema.const)) errors.push(`${path} debe ser ${JSON.stringify(schema.const)}`);
  if (schema.enum && !schema.enum.some((item) => deepEqual(item, value))) errors.push(`${path} no pertenece al enum permitido`);
  if (schema.type && !matchesType(schema.type, value)) {
    errors.push(`${path} debe ser de tipo ${schema.type}`);
    return errors;
  }
  if (typeof value === "string") {
    if (schema.minLength !== undefined && value.length < schema.minLength) errors.push(`${path} es demasiado corto`);
    if (schema.maxLength !== undefined && value.length > schema.maxLength) errors.push(`${path} es demasiado largo`);
    if (schema.pattern && !(new RegExp(schema.pattern)).test(value)) errors.push(`${path} no cumple ${schema.pattern}`);
  }
  if (typeof value === "number") {
    if (schema.minimum !== undefined && value < schema.minimum) errors.push(`${path} debe ser >= ${schema.minimum}`);
    if (schema.exclusiveMinimum !== undefined && value <= schema.exclusiveMinimum) errors.push(`${path} debe ser > ${schema.exclusiveMinimum}`);
  }
  if (Array.isArray(value)) {
    if (schema.minItems !== undefined && value.length < schema.minItems) errors.push(`${path} requiere al menos ${schema.minItems} elementos`);
    if (schema.maxItems !== undefined && value.length > schema.maxItems) errors.push(`${path} supera ${schema.maxItems} elementos`);
    if (schema.uniqueItems) {
      const unique = new Set(value.map((item) => JSON.stringify(item)));
      if (unique.size !== value.length) errors.push(`${path} contiene duplicados`);
    }
    if (schema.items) value.forEach((item, index) => validateSchema(schema.items, item, `${path}[${index}]`, errors));
  }
  if (isObject(value)) {
    for (const required of schema.required ?? []) if (!(required in value)) errors.push(`${path}.${required} es requerido`);
    const properties = schema.properties ?? {};
    for (const [key, item] of Object.entries(value)) {
      if (properties[key]) validateSchema(properties[key], item, `${path}.${key}`, errors);
      else if (schema.additionalProperties === false) errors.push(`${path}.${key} no está permitido`);
      else if (isObject(schema.additionalProperties)) validateSchema(schema.additionalProperties, item, `${path}.${key}`, errors);
    }
  }
  return errors;
}
function matchesType(type, value) {
  if (Array.isArray(type)) return type.some((item) => matchesType(item, value));
  return type === "object" ? isObject(value) : type === "array" ? Array.isArray(value) : type === "integer" ? Number.isInteger(value) : type === "number" ? typeof value === "number" && Number.isFinite(value) : type === "null" ? value === null : typeof value === type;
}
function isObject(value) { return value !== null && typeof value === "object" && !Array.isArray(value); }
function deepEqual(a, b) { return JSON.stringify(a) === JSON.stringify(b); }
