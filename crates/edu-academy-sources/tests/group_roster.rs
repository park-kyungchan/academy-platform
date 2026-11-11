//! Synthetic day-record pages only. Invented people and fictional keys.
use edu_academy_sources::{
    data::*,
    http::{Client, GroupRoster, ORIGIN, Request, Response, Transport},
};
use std::collections::VecDeque;

const DATE: &str = "2040-01-04";
const NOW: &str = "2040-01-04T09:00:00.000Z";
const COVERAGE: &str =
    "scheduled_rows_of_group_on_date_not_enrolment_not_corroborated_by_selected_record";

struct Fictional {
    replies: VecDeque<String>,
    requests: Vec<Request>,
}
impl Transport for Fictional {
    fn dispatch(&mut self, request: &Request) -> Result<Response> {
        self.requests.push(request.clone());
        let body = self.replies.pop_front().expect("finite synthetic page");
        Ok(Response {
            status: 200,
            headers: vec![],
            bytes: body.into_bytes(),
            url: format!("{ORIGIN}{}", request.path),
            stalled: false,
        })
    }
}
fn client(pages: Vec<String>) -> Client<Fictional> {
    let mut client = Client::new(
        obj(&[
            ("teacherLabel", "Fictional Teacher".into()),
            ("teacherKey", "99001".into()),
            ("collegeKey", "88001".into()),
        ]),
        Fictional {
            replies: pages.into(),
            requests: vec![],
        },
    )
    .unwrap();
    client.authenticated = true;
    client
}
fn row(name: &str, id: &str, record: &str) -> String {
    format!(
        r#"<tr><td><a onclick="onOpenDaysMain('{id}')">{name}</a></td><td>Fictional course</td><td></td><td></td><td></td><td></td><td></td><td></td><td><select onchange="onAttn('0','5','{id}','{record}','8')"><option value=Y>Fictional</option></select></td><td></td><td><textarea onfocusout="onFoPrg('0','5','{id}','{record}','8')">100%</textarea></td><td><textarea onfocusout="onFoHw('0','5','{id}','{record}','8')">Fictional</textarea></td><td><input onfocusout="onFoMm('0','5','{id}','{record}','8')" value="Fictional memo"></td></tr>"#
    )
}
fn page(teacher: &str, selected: &str, rows: String) -> String {
    format!(
        r#"<input name=std_date value="{DATE}"><input name=teacher_pri_no value={teacher}><select name=grp_seq><option value=3{}>Fictional group A</option><option value=4{}>Fictional group B</option></select>
<table><thead><tr><th rowspan=2>이름</th><th rowspan=2>과정</th><th rowspan=2>지속사항</th><th colspan=4>지난진도</th><th colspan=6>일괄 발송 금일진도</th></tr><tr><th>지난수업일</th><th>진도</th><th>숙제</th><th>메모</th><th></th><th>금일출석</th><th>일일테스트</th><th>진도</th><th>숙제</th><th>메모</th></tr></thead><tbody>{rows}</tbody></table>
<script>function onAttn(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onFoPrg(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onFoHw(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onFoMm(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onOpenDaysMain(student_pri_no){{}}</script>"#,
        if selected == "3" { " selected" } else { "" },
        if selected == "4" { " selected" } else { "" },
    )
}
fn good(rows: String) -> String {
    page("99001", "3", rows)
}
fn assert_one_request(client: &Client<Fictional>) {
    assert_eq!(client.transport.requests.len(), 1);
    let request = &client.transport.requests[0];
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.path,
        "/servlet/controller.cct.tutor.DayRecordServlet"
    );
    assert!(request.body.contains(&("grp_seq".into(), "3".into())));
}
fn assert_refusal(pages: Vec<String>, code: &str) {
    let mut client = client(pages);
    let error = client.read_group_roster(DATE, "3").unwrap_err();
    assert_eq!(error.message, code);
    assert_one_request(&client);
}

#[test]
fn roster_of_three_in_sorted_order_with_exactly_one_request() {
    let rows = format!(
        "{}{}{}",
        row("가상학생C", "77003", "15"),
        row("가상학생A", "77001", "13"),
        row("가상학생B", "77002", "14")
    );
    let mut client = client(vec![good(rows)]);
    let roster: GroupRoster = client.read_group_roster(DATE, "3").unwrap();
    assert_eq!(roster.date, DATE);
    assert_eq!(roster.group_id, "3");
    assert_eq!(roster.teacher_key, "99001");
    assert_eq!(roster.coverage, COVERAGE);
    assert_eq!(
        roster
            .students
            .iter()
            .map(|s| (s.student_name.as_str(), s.student_key.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("가상학생A", "77001"),
            ("가상학생B", "77002"),
            ("가상학생C", "77003")
        ]
    );
    assert_eq!(roster.students[0].course_key, "5");
    assert_eq!(roster.students[0].record_key, "13");
    assert_eq!(roster.students[0].membership_key, "8");
    assert_one_request(&client);
}

#[test]
fn empty_valid_table_returns_empty_roster() {
    let mut client = client(vec![good(String::new())]);
    let roster = client.read_group_roster(DATE, "3").unwrap();
    assert!(roster.students.is_empty());
    assert_eq!(roster.coverage, COVERAGE);
    assert_one_request(&client);
}

#[test]
fn wrong_teacher_key_on_page_refuses_after_one_request() {
    assert_refusal(
        vec![page("99002", "3", row("가상학생A", "77001", "13"))],
        "day_record_scope_conflict",
    );
}

#[test]
fn page_group_different_from_request_refuses_after_one_request() {
    assert_refusal(
        vec![page("99001", "4", row("가상학생A", "77001", "13"))],
        "day_record_scope_conflict",
    );
}

#[test]
fn duplicated_row_refuses_after_one_request() {
    let duplicated = row("가상학생A", "77001", "13");
    assert_refusal(
        vec![good(format!("{duplicated}{duplicated}"))],
        "ambiguous_day_record_identity",
    );
}

#[test]
fn one_name_with_different_keys_refuses_after_one_request() {
    assert_refusal(
        vec![good(format!(
            "{}{}",
            row("가상학생A", "77001", "13"),
            row("가상학생A", "77002", "14")
        ))],
        "ambiguous_day_record_identity",
    );
}

#[test]
fn sixty_one_rows_refuse_after_one_request() {
    let names = ["가상학생A", "가상학생B", "가상학생C"];
    let rows = (0..61)
        .map(|i| {
            row(
                names[i % 3],
                &format!("{}", 77001 + i),
                &format!("{}", 13 + i),
            )
        })
        .collect::<String>();
    assert_refusal(vec![good(rows)], "day_record_roster_rows_rejected");
}

#[test]
fn page_without_table_refuses_after_one_request() {
    let page = format!(
        r#"<input name=std_date value="{DATE}"><input name=teacher_pri_no value=99001><select name=grp_seq><option value=3 selected>Fictional group A</option></select>"#
    );
    assert_refusal(vec![page], "day_record_column_contract_rejected");
}

#[test]
fn thirteen_group_options_refuse_for_list_groups_of_date() {
    let options = (1..=13)
        .map(|i| {
            format!(
                "<option value={i}{}>Fictional group {i}</option>",
                if i == 1 { " selected" } else { "" }
            )
        })
        .collect::<String>();
    let page = format!(
        r#"<input name=std_date value="{DATE}"><input name=teacher_pri_no value=99001><select name=grp_seq>{options}</select>"#
    );
    let mut client = client(vec![page]);
    let error = client.list_groups_of_date(DATE, NOW).unwrap_err();
    assert_eq!(error.message, "day_record_group_options_rejected");
    assert_eq!(client.transport.requests.len(), 1);
}

#[test]
fn list_groups_of_date_returns_discover_groups_with_one_request() {
    let mut client = client(vec![good(String::new())]);
    let result = client.list_groups_of_date(DATE, NOW).unwrap();
    assert_eq!(result.get("teacherKey"), &"99001".into());
    assert_eq!(result.get("groups").array().len(), 2);
    assert_eq!(result.get("selectedGroupId"), &"3".into());
    assert_eq!(client.transport.requests.len(), 1);
}
