/** Presentation of the shared application's actual connection result, not a permission gate. */
export function connectionView(value:unknown){
  const source=value as {schemaVersion?:unknown;retainedQuery?:unknown;acquisition?:{transport?:unknown;state?:unknown;reason?:unknown;browserFallback?:unknown};officialJournalSave?:unknown;parentSend?:unknown}|null|undefined;
  const known=source?.schemaVersion===1&&source.retainedQuery==='available'
    &&source.acquisition?.transport==='direct_http_api'&&source.acquisition.state==='blocked'
    &&source.acquisition.reason==='direct_http_authentication_not_qualified'&&source.acquisition.browserFallback===false
    &&source.officialJournalSave==='not_implemented'&&source.parentSend==='teacher_owned';
  return {
    retainedLabel:known?'보존 자료 조회 가능 · 조회 시각은 학원 갱신 시각이 아닙니다.':'보존 자료 연결 상태 미확인',
    acquisitionLabel:known?'실시간 학원 조회 보류 · 직접 HTTP 인증 연결 필요 · 브라우저로 우회하지 않습니다.':'학원 연결 상태 미확인 · 갱신을 활성화하지 않습니다.',
    journalLabel:known?'학원 일지 저장 미지원 · 초안과 최종 저장은 별개입니다.':'학원 일지 저장 연결 상태 미확인',
    sendLabel:'학부모 최종 점검·전송은 선생님이 진행합니다.',
    refreshDisabled:true,
  };
}
