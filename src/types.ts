export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonValue[] | JsonObject;
export type JsonObject = { [key: string]: JsonValue | undefined };

export type OpenCodeConfig = {
  plugin?: string | string[];
  [key: string]: JsonValue | undefined;
};

export interface DoctorCheck {
  name: string;
  status: "PASS" | "WARN" | "FAIL";
  message: string;
}

export type AutoshipConfig = {
  runtime?: "opencode" | string;
  maxConcurrentAgents?: number | string;
  max_agents?: number | string;
  plannerModel?: string;
  coordinatorModel?: string;
  orchestratorModel?: string;
  reviewerModel?: string;
  leadModel?: string;
  models?: string[];
  labels?: string[];
  refreshModels?: boolean;
  [key: string]: JsonValue | undefined;
};

export type ModelRoutingModel = {
  id: string;
  cost?: "free" | "go" | "selected" | string;
  strength?: number;
  max_task_types?: string[];
  enabled?: boolean;
  [key: string]: JsonValue | undefined;
};

export type ModelRoutingPool = {
  description?: string;
  models?: string[];
  [key: string]: JsonValue | undefined;
};

export type ModelRouting = {
  roles?: Record<string, string>;
  pools?: Record<string, ModelRoutingPool>;
  defaultFallback?: string | null;
  models?: ModelRoutingModel[];
  [key: string]: JsonValue | undefined;
};
