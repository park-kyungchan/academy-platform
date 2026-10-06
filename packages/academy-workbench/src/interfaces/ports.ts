import type { ActionName, ActionRequest, ActionResult, TrustedPrincipal } from '../contracts/actions.ts';
import type { SyntheticApplicationService } from '../application/service.ts';
/** Future human UI and agent semantic tools share this exact permission-checked boundary. */
export interface ApplicationPort {
  invoke<K extends ActionName>(request: ActionRequest<K>): ActionResult<K>;
}
export function bindTrustedSession(service: SyntheticApplicationService, principal: TrustedPrincipal): ApplicationPort {
  const trusted = structuredClone(principal);
  return { invoke: request => service.execute(trusted, request) };
}
export interface HumanUIPort extends ApplicationPort {}
export interface AgentSemanticToolsPort extends ApplicationPort {}
// Optional future WebMCP adapter belongs only to the user's own app.
// No WebMCP registration, third-party page injection, API routes or server here.
