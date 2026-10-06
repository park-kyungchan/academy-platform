import { SyntheticCollector } from '../src/collectors/adapter.ts';
import { SyntheticApplicationService } from '../src/application/service.ts';
import { bindTrustedSession } from '../src/interfaces/ports.ts';
import { syntheticSnapshot } from '../fixtures/synthetic.ts';
import { CONTRACT_REVISION } from '../src/domain/canonical.ts';
const service = new SyntheticApplicationService(await new SyntheticCollector(syntheticSnapshot).collect());
const ui = bindTrustedSession(service, { actorId: 'synthetic-teacher', role: 'teacher', academyIds: ['synthetic-academy'] });
console.log(JSON.stringify(ui.invoke({ action: 'lookupStudent', params: { studentId: 'synthetic-student-1' }, academyId: 'synthetic-academy', idempotencyKey: 'demo-read-1', contractsRevision: CONTRACT_REVISION }), null, 2));
