"""Expand the evaluation case set from 30 to 120 cases (REQ-326).

The parent contract's target distribution is 120 cases; the pilot shipped a
30-case quarter of it. This tool authors the remaining 90 internally authored
cases at the exact genre quotas (family 20; urban_romance/revenge/suspense 16
each; workplace/rural/comedy 12 each; historical/cross_genre 8 each — the
pilot's quotas times four) and the 30:30:24:12 split ratio, with holdout still
sealed at zero for v1.

Premise families follow the corpus convention: the family names the dramatic
mechanism, not the props — `guardian_secret_ledger` covers a mother's monthly
transfer and a father's eight-year "equipment fee" alike. Families never cross
splits. `hard_slice` stays null on every new case (each genre keeps exactly
one marker, already present in the pilot), and every case gets a fresh
license id.

The tool appends to the split files; `split_cases.py` remains the single
authority for split membership and must be run afterwards (its ASSIGNMENT
table gains the new ids).

Usage (from the repository root):
    python eval/tools/expand_cases_to_120.py
"""

from __future__ import annotations

import json
from collections import Counter
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
CASES = ROOT / "eval" / "cases"
SPLIT_USES = {
    "dev": ["evaluation"],
    "train": ["evaluation", "skill_derivation"],
    "validation": ["evaluation"],
    "challenge": ["evaluation"],
}

GENRE_QUOTAS = {
    "family": 15,
    "urban_romance": 12,
    "revenge": 12,
    "suspense": 12,
    "workplace": 9,
    "rural": 9,
    "comedy": 9,
    "historical": 6,
    "cross_genre": 6,
}
SPLIT_ADDITIONS = {"dev": 28, "train": 28, "validation": 23, "challenge": 11}

# genre -> number of existing cases, so new ids continue the per-genre sequence.
EXISTING_IDS = {
    "family": 5,
    "urban_romance": 4,
    "revenge": 4,
    "suspense": 4,
    "workplace": 3,
    "rural": 3,
    "comedy": 3,
    "historical": 2,
    "cross_genre": 2,
}

# Every entry: (family, split, genre, difficulty, input, required, forbidden,
#               episodes, locations, cast, audience)
NEW_CASES: list[tuple[str, str, str, str, str, list, list, int, int, int, str]] = [
    # ---------------- dev (+28) ----------------
    # family
    ("guardian_secret_ledger", "dev", "family", "ambiguous",
     "母亲每月十五号都会给同一个陌生账户转两千块，退休金已经见底。",
     ["转账记录", "存折"], ["失忆", "梦醒"], 8, 4, 6, "25-45"),
    ("guardian_secret_ledger", "dev", "family", "ordinary",
     "父亲的存折上有一笔付了八年的“设备费”，收款方是一家早已注销的疗养院。",
     ["存折", "设备费"], ["突然继承遗产", "恶毒后妈"], 8, 4, 6, "25-50"),
    ("sibling_swap_debt", "dev", "family", "hard",
     "当年替哥哥去体检的人是弟弟，如今哥哥的病急需那份真实的体检记录。",
     ["体检报告", "兄弟"], ["失忆", "双胞胎互换人生"], 10, 4, 6, "25-45"),
    ("sibling_swap_debt", "dev", "family", "ordinary",
     "姐姐顶替妹妹的名字进厂三十年，退休那年档案对不上，养老金面临重算。",
     ["人事档案", "工牌"], ["一夜暴富", "恶毒亲戚"], 8, 5, 7, "30-50"),
    # urban_romance
    ("reunion_through_obligation", "dev", "urban_romance", "ordinary",
     "分手五年后，两人被一套迟迟没过户的老房子重新锁在一起。",
     ["老房子", "过户"], ["车祸失忆", "豪门联姻"], 10, 4, 6, "22-35"),
    ("reunion_through_obligation", "dev", "urban_romance", "ambiguous",
     "她负责注销他公司的工商档案，才发现紧急联系人一栏写着自己。",
     ["注销手续", "紧急联系人"], ["假死", "隐藏首富"], 8, 4, 5, "22-35"),
    ("contracted_role_reversal", "dev", "urban_romance", "ambiguous",
     "婚礼上雇来充场面的“男友”，其实是酒店新来的投资人本人。",
     ["婚礼", "合同"], ["真实身份是明星", "失忆"], 10, 4, 7, "20-40"),
    ("contracted_role_reversal", "dev", "urban_romance", "hard",
     "她雇他假扮丈夫应付催婚，条件是不许动心，先违约的人是他。",
     ["催婚", "协议"], ["豪门恩怨", "替嫁"], 12, 5, 7, "22-40"),
    # revenge
    ("exposure_via_own_rules", "dev", "revenge", "hard",
     "她被“末位淘汰”出局，三年后以合规审计的身份回来查当年的裁员程序。",
     ["裁员名单", "审计"], ["主角黑化杀人", "失忆"], 12, 5, 7, "25-45"),
    ("exposure_via_own_rules", "dev", "revenge", "ordinary",
     "他用师父亲手签的师徒协议条款，在行业大赛上夺回被顶替的署名。",
     ["师徒协议", "署名"], ["当众捅人", "毁容"], 8, 4, 6, "25-50"),
    ("benefactor_unmasked", "dev", "revenge", "hard",
     "资助她读完大学的匿名捐款人，正是当年撞伤她父亲后逃逸的人。",
     ["汇款单", "事故认定书"], ["失忆", "同归于尽"], 12, 5, 6, "25-45"),
    ("benefactor_unmasked", "dev", "revenge", "ambiguous",
     "收留他三年的远房叔叔，正是当年害他家破产的合伙人。",
     ["账本", "合伙协议"], ["主角入狱", "替身"], 10, 5, 6, "30-50"),
    # suspense
    ("second_copy_of_recording", "dev", "suspense", "ambiguous",
     "她在庭上播放的行车记录仪之外，还有第二份拷贝在敲诈者手里。",
     ["行车记录仪", "拷贝"], ["凶手是双胞胎", "梦游作案"], 10, 4, 6, "25-45"),
    ("second_copy_of_recording", "dev", "suspense", "hard",
     "医院监控被人剪过，原始片段却出现在实习医生的私人硬盘里。",
     ["监控录像", "硬盘"], ["鬼魂作案", "催眠犯罪"], 12, 5, 7, "25-50"),
    ("witness_close_kin", "dev", "suspense", "hard",
     "唯一能证明她清白的目击者，是她刚再婚的继父。",
     ["目击证词", "再婚"], ["继父是真凶", "失忆破案"], 12, 4, 6, "25-45"),
    ("witness_close_kin", "dev", "suspense", "ordinary",
     "全楼都知道那晚谁进了电梯，三位物业师傅却约好了一样沉默。",
     ["电梯监控", "值班表"], ["灵异事件", "连环杀手"], 8, 4, 7, "20-45"),
    # workplace
    ("successor_judges_predecessor", "dev", "workplace", "ambiguous",
     "她接手的门店背着前任店长留下的烂账，而前任现在是她的区域总监。",
     ["交接清单", "烂账"], ["主角辞职爽文", "老板恋爱"], 10, 4, 7, "25-45"),
    ("successor_judges_predecessor", "dev", "workplace", "hard",
     "新主管要给“过渡期表现”打分，打分对象是带他入行的师傅。",
     ["打分表", "师傅"], ["恶性竞争下毒", "突然上市"], 10, 4, 6, "25-50"),
    ("reference_chain_forgery", "dev", "workplace", "ambiguous",
     "实习生的推荐信层层属实，最后一级的签名人是她已故的父亲。",
     ["推荐信", "签名"], ["鬼魂显灵", "AI换脸"], 8, 4, 5, "22-40"),
    # rural
    ("boundary_stone_missing", "dev", "rural", "ambiguous",
     "两家争了三十年的老槐树一夜被砍，树桩下的界石不翼而飞。",
     ["老槐树", "界石"], ["村霸打人致死", "寻宝"], 10, 4, 7, "30-50"),
    ("boundary_stone_missing", "dev", "rural", "hard",
     "修高速征地，爷爷拿出的地契上写着一个任何地图都查不到的小地名。",
     ["地契", "征地"], ["拆迁暴富", "文物走私"], 12, 5, 7, "30-50"),
    ("roof_layer_letters", "dev", "rural", "ordinary",
     "返乡青年翻新老屋，在房顶夹层里发现前屋主留下的一箱没有寄出的信。",
     ["老屋", "信件"], ["鬼屋", "宝藏地图"], 8, 4, 5, "25-50"),
    # comedy
    ("mistaken_professional", "dev", "comedy", "ordinary",
     "他只是来修水管的，却被全家当成重金请来的婚姻调解师，不敢开口澄清。",
     ["工具箱", "调解"], ["真变调解大师", "打架"], 8, 4, 7, "20-45"),
    ("mistaken_professional", "dev", "comedy", "ambiguous",
     "她客串了一天陪诊，被家属当成私人医生言听计从，连手术同意书都递了过来。",
     ["陪诊", "家属"], ["真医生身份", "医疗事故"], 8, 4, 6, "25-50"),
    ("inheritance_pet_clause", "dev", "comedy", "ordinary",
     "遗嘱把房子留给猫，条件是“猫认可的人”才能继续住，全家开始讨好一只猫。",
     ["遗嘱", "猫"], ["猫会说话", "宠物成精"], 8, 4, 6, "20-45"),
    # historical
    ("artisan_signature_debt", "dev", "historical", "hard",
     "落选匠人把自己的手艺署上师兄之名送进官窑，多年后师兄被召来复刻“自己”的杰作。",
     ["官窑", "款识"], ["穿越", "宫斗爱情"], 12, 4, 6, "25-50"),
    ("artisan_signature_debt", "dev", "historical", "ambiguous",
     "替考进书院的寒生，遇到了执着于“当年那篇文章”的座师。",
     ["考卷", "座师"], ["穿越", "皇子夺嫡"], 10, 4, 6, "25-50"),
    # cross_genre
    ("perfect_match_knows_too_much", "dev", "cross_genre", "hard",
     "相亲对象各方面都完美，唯一的破绽是他知道她从未告诉任何人的过敏原。",
     ["相亲", "过敏原"], ["读心术", "特工"], 10, 4, 5, "22-40"),

    # ---------------- train (+28) ----------------
    # family
    ("elder_remarriage_shadow", "train", "family", "ambiguous",
     "六十岁的母亲要再婚，对象是当年父亲工友会上最能喝的那一位。",
     ["再婚", "工友"], ["子女逼婚", "遗产大战"], 8, 5, 7, "30-50"),
    ("elder_remarriage_shadow", "train", "family", "hard",
     "父亲的黄昏恋对象，是子女小学时的班主任，客气里全是旧账。",
     ["班主任", "旧账"], ["恶毒后妈", "夺产"], 10, 5, 7, "30-50"),
    ("naming_rights_war", "train", "family", "ordinary",
     "孙子随母姓的口头约定，在满月酒前一天被爷爷奶奶单方面反悔。",
     ["满月酒", "姓名"], ["离婚收场", "夺子"], 8, 5, 8, "25-45"),
    ("naming_rights_war", "train", "family", "ambiguous",
     "两家人给孩子起的名字里，各藏着一位已故长辈的名讳。",
     ["名字", "家谱"], ["双胞胎调包", "恶亲绑架"], 8, 5, 8, "25-45"),
    # urban_romance
    ("thin_wall_neighbors", "train", "urban_romance", "ambiguous",
     "隔壁每晚十一点准时的哭声，让她终于敲开了那扇门。",
     ["哭声", "隔壁"], ["凶宅", "绝症"], 10, 3, 5, "22-35"),
    ("thin_wall_neighbors", "train", "urban_romance", "hard",
     "合租室友贴着“勿动”的储物间里，锁着她失踪三年前男友的画具。",
     ["储物间", "画具"], ["藏尸", "双胞胎"], 12, 3, 5, "22-35"),
    ("eighteen_flights_daily", "train", "urban_romance", "ordinary",
     "电梯坏了三个月，她和他每天爬十八层，谁都没先开口。",
     ["电梯", "楼梯间"], ["英雄救美", "前未婚妻回归"], 8, 3, 5, "22-40"),
    # revenge
    ("slow_replacement", "train", "revenge", "hard",
     "徒弟一步步接管师父的客源、口碑和家宴座位，师父最后一样都留不住。",
     ["客源", "拜师宴"], ["徒弟下毒", "同归于尽"], 12, 5, 7, "30-50"),
    ("slow_replacement", "train", "revenge", "ambiguous",
     "新同事“热心”接走了她全部客户，回头连工牌都换成了对方的名字。",
     ["客户名单", "工牌"], ["职场性侵", "砍人"], 10, 4, 6, "25-45"),
    ("bid_by_one_cent", "train", "revenge", "ambiguous",
     "她匿名给仇人的公司投了十年标，每次都以一分钱之差输给自己安排的人。",
     ["投标", "一分钱"], ["商业间谍入狱", "黑帮"], 12, 5, 6, "25-50"),
    # suspense
    ("too_tidy_alibi", "train", "suspense", "hard",
     "全组加班的监控里，只有他每二十分钟准时消失八分钟。",
     ["加班监控", "考勤"], ["多重人格", "鬼上身"], 12, 4, 7, "25-45"),
    ("too_tidy_alibi", "train", "suspense", "ambiguous",
     "她的通勤记录精确到分钟，唯独案发那天连手机都“恰好”没电。",
     ["通勤记录", "手机"], ["临时疯癫", "双胞胎互换"], 10, 4, 6, "25-50"),
    ("gate_knows_dead_id", "train", "suspense", "hard",
     "小区人脸闸机认得一个身份已经注销三年的人。",
     ["人脸闸机", "注销"], ["复制人", "诈尸"], 10, 4, 6, "20-45"),
    # workplace
    ("system_vs_master", "train", "workplace", "ambiguous",
     "系统判定老师傅“人效过低”，执行优化的按钮在他带出来的徒弟手里。",
     ["人效报表", "优化名单"], ["徒弟举报上瘾", "集体辞职"], 10, 4, 7, "25-50"),
    ("system_vs_master", "train", "workplace", "hard",
     "客服之星的录音被掐头去尾挂上内网，逐字稿能还她清白，却没人去调。",
     ["录音", "逐字稿"], ["网暴致死", "CEO恋情"], 10, 4, 6, "22-40"),
    ("anonymous_wall_ids", "train", "workplace", "ordinary",
     "公司匿名吐槽墙背后，运维看得到每一个“匿名”作者的工号。",
     ["吐槽墙", "工号"], ["数据泄露上市", "黑客大战"], 8, 4, 6, "22-40"),
    # rural
    ("dividend_ledger_gap", "train", "rural", "hard",
     "合作社首次分红，账面盈利比收购站的流水整整少了三成。",
     ["分红", "流水账"], ["村干部灭口", "私藏金矿"], 12, 5, 8, "30-50"),
    ("dividend_ledger_gap", "train", "rural", "ambiguous",
     "全村按手印入股的果园，合同上的亩数比实际多出四十亩。",
     ["手印", "合同"], ["强拆", "黑社会"], 10, 4, 8, "30-50"),
    ("drone_vs_almanac", "train", "rural", "ordinary",
     "无人机植保队进村作业，爷爷坚持要按老黄历上的吉日才肯放行。",
     ["无人机", "老黄历"], ["无人机坠毁伤人", "封建害人致死"], 8, 4, 7, "30-50"),
    # comedy
    ("wrong_window_message", "train", "comedy", "ordinary",
     "母亲把吐槽合集错发进家族群，撤回倒计时开始，全家截图已停不下来。",
     ["家族群", "撤回"], ["群主是隐藏富豪", "真人秀直播"], 8, 4, 7, "20-45"),
    ("wrong_window_message", "train", "comedy", "ambiguous",
     "他在大群里手滑发出给老板起的外号，两分钟的撤回竞速改变了他的职业生涯。",
     ["外号", "大群"], ["被开除后创业暴富", "打官司"], 8, 4, 7, "22-40"),
    ("white_lie_sticky", "train", "comedy", "ambiguous",
     "他在婚宴上随口编的养生建议被亲戚当真，一年后全家按他的“偏方”生活。",
     ["养生建议", "亲戚"], ["查出重病", "成网红神医"], 10, 5, 8, "25-50"),
    ("white_lie_sticky", "train", "comedy", "hard",
     "她为推掉饭局谎称“人在国外”，朋友们做出了定位地图开始全网找她。",
     ["饭局", "定位"], ["真失踪", "绑架"], 10, 4, 7, "22-40"),
    # historical
    ("delayed_decade_letter", "train", "historical", "hard",
     "戍边十年的家书抵达时，妻子已替“阵亡”的他守了十年灵位。",
     ["家书", "灵位"], ["穿越", "起兵造反"], 12, 4, 6, "25-50"),
    ("delayed_decade_letter", "train", "historical", "ordinary",
     "迟到二十年的科举喜报送到，早已认命的私塾先生被重新卷进功名漩涡。",
     ["喜报", "私塾"], ["穿越", "朝堂权谋"], 10, 4, 6, "25-50"),
    # cross_genre
    ("kindness_audit_collision", "train", "cross_genre", "hard",
     "她十年匿名资助的学生，出现在她被调查公司的审计名单上。",
     ["资助记录", "审计名单"], ["学生复仇杀人", "特赦"], 12, 5, 6, "25-45"),
    ("route_through_her_door", "train", "cross_genre", "ordinary",
     "救了她一命的陌生快递员，此后每一单都“恰好”经过她家楼下。",
     ["快递员", "配送路线"], ["跟踪狂杀人", "超能力"], 10, 4, 5, "22-40"),
    ("future_dated_photo", "train", "cross_genre", "hard",
     "家族相册里多出一张没人记得拍过的全家福，冲印日期是下个月。",
     ["相册", "冲印日期"], ["时间旅行机", "鬼照片"], 10, 4, 6, "20-45"),

    # ---------------- validation (+23) ----------------
    # family
    ("donor_sibling_truth", "validation", "family", "hard",
     "配型成功救姐姐的孩子，是当年在医院抱错抱回来的那个。",
     ["配型报告", "出生记录"], ["抱错孩子互撕", "法庭大战"], 12, 5, 7, "25-45"),
    ("donor_sibling_truth", "validation", "family", "ambiguous",
     "为救老二而生的老三，十八年后开始追问自己出生的理由。",
     ["老三", "脐带血"], ["离家出走生死不明", "父母双亡"], 10, 5, 7, "25-45"),
    ("recipe_inheritance_refusal", "validation", "family", "ambiguous",
     "祖传酱方只传儿媳不传女，女儿带着一份复印配方要求全家摊牌。",
     ["酱方", "儿媳"], ["秘方拍卖", "毒倒全家"], 10, 4, 7, "30-50"),
    ("recipe_inheritance_refusal", "validation", "family", "ordinary",
     "老字号招牌菜的秘密原料被吃素的继承人停用，老顾客集体上门讨说法。",
     ["招牌菜", "继承人"], ["食物中毒", "店铺拆迁"], 8, 4, 7, "30-50"),
    # urban_romance
    ("unsign_the_past", "validation", "urban_romance", "ambiguous",
     "散伙饭那晚没签完的分手协议和股权转让书，压在同一张桌上。",
     ["分手协议", "股权"], ["复合即大结局", "商战吞并"], 10, 4, 6, "25-45"),
    ("unsign_the_past", "validation", "urban_romance", "hard",
     "他注销了两人共建十年的歌单账号，她用本地备份一首一首找回。",
     ["歌单", "备份"], ["乐坛重逢", "绝症"], 8, 3, 5, "22-35"),
    ("unsent_mail_bounce", "validation", "urban_romance", "ordinary",
     "她每天给已读不回的人写一封不发送的邮件，直到系统提示对方已更换邮箱。",
     ["邮件", "已读不回"], ["对方去世", "婚礼抢婚"], 8, 3, 5, "22-35"),
    # revenge
    ("auction_buyback", "validation", "revenge", "hard",
     "被贱卖的祖宅出现在拍卖行，她以匿名买家身份一路加价到对方资金链断裂。",
     ["拍卖", "祖宅"], ["纵火", "当众羞辱打人"], 12, 5, 7, "25-50"),
    ("debt_patience", "validation", "revenge", "ambiguous",
     "他花十年悄悄收购仇人公司的外围债权，只等一次挤兑到来。",
     ["债权", "挤兑"], ["绑架", "股市操纵入狱"], 12, 5, 6, "30-50"),
    ("defense_chair_return", "validation", "revenge", "ordinary",
     "她成为母校史上最年轻的答辩主席，台下坐着当年劝她“别读了”的系主任。",
     ["答辩", "系主任"], ["当众撕证", "学阀灭口"], 8, 4, 6, "25-45"),
    # suspense
    ("child_drawing_window", "validation", "suspense", "hard",
     "孩子的画里反复出现同一扇窗，而那扇窗五年前就被封死了。",
     ["画", "封死的窗"], ["灵异真鬼", "通灵"], 12, 4, 6, "25-45"),
    ("extra_artwork_name", "validation", "suspense", "ambiguous",
     "幼儿园手工课上多出一件作品，署名是花名册上不存在的小朋友。",
     ["手工课", "花名册"], ["鬼童", "拐卖团伙"], 10, 4, 7, "25-45"),
    ("porch_light_no_one", "validation", "suspense", "ordinary",
     "每晚十一点，玄关的感应灯为一扇没人进出的门亮起。",
     ["感应灯", "玄关"], ["闹鬼", "小偷一家"], 8, 3, 5, "25-45"),
    # workplace
    ("handover_missing_line", "validation", "workplace", "hard",
     "离职同事留下的“最优流程文档”，每一版都恰好删掉同一行关键步骤。",
     ["流程文档", "版本记录"], ["数据大爆炸", "主角挖角创业"], 10, 4, 6, "25-45"),
    ("no_one_signs_here", "validation", "workplace", "ambiguous",
     "部门最能干的人突然申请调岗，交接清单最后夹着一份没人敢签的字。",
     ["调岗申请", "交接清单"], ["贪腐大案", "坠楼"], 10, 4, 6, "25-50"),
    # rural
    ("flood_ledger_resurfaced", "validation", "rural", "hard",
     "洪水冲开的旧砖窖里，是当年生产队“丢失”的储备粮账本。",
     ["砖窖", "账本"], ["浮尸", "日军宝藏"], 12, 4, 7, "30-50"),
    ("dry_well_testimony", "validation", "rural", "ambiguous",
     "修族谱时，两位九十岁老人对“当年谁救过谁”各执一词，都指向同一口枯井。",
     ["族谱", "枯井"], ["井下藏尸", "宝藏"], 10, 4, 6, "30-50"),
    # comedy
    ("speech_swap_wedding", "validation", "comedy", "ordinary",
     "婚礼上伴郎和伴娘恰好是彼此的“前任见证人”，两份致辞稿拿串了。",
     ["致辞稿", "伴郎伴娘"], ["婚礼取消", "前任复合"], 8, 4, 8, "22-40"),
    ("seat_one_war", "validation", "comedy", "ambiguous",
     "喜宴主桌的长辈座次排错了一位，两位舅爷为“上座”展开全天博弈。",
     ["主桌", "座次"], ["婚宴斗殴", "酒精致死"], 8, 4, 9, "25-50"),
    # historical
    ("river_drawn_wrong", "validation", "historical", "hard",
     "地图匠把界河画偏了一笔，两个村子为一滩芦苇荡打了六十年官司。",
     ["界河", "芦苇荡"], ["村落械斗致死", "钦差微服"], 12, 4, 7, "25-50"),
    ("prescription_misread", "validation", "historical", "ambiguous",
     "药铺学徒抄错一味药名，一百年后这张老方子成了医馆的悬案。",
     ["药方", "药名"], ["神医金手指", "宫廷毒杀"], 10, 4, 6, "25-50"),
    # cross_genre
    ("memory_restorer_blind_spot", "validation", "cross_genre", "hard",
     "旧货市场的“回忆修复师”能修好任何旧物，唯独修不了自己那只手表。",
     ["旧物", "手表"], ["读心", "永生者"], 10, 4, 5, "22-40"),
    ("one_thing_to_take", "validation", "cross_genre", "ambiguous",
     "老街拆迁前社区征集“一件必须带走的东西”，一百二十份清单拼出一桩无人报案的失踪。",
     ["清单", "拆迁"], ["灵异搬迁", "连环杀手"], 12, 5, 7, "25-45"),

    # ---------------- challenge (+11) ----------------
    # family
    ("stranger_pays_columbarium", "challenge", "family", "hard",
     "父亲的骨灰位管理费年年有人续交，缴费人的名字全家无人认识。",
     ["骨灰位", "缴费单"], ["阴婚", "诈骗集团"], 12, 4, 6, "25-50"),
    ("stranger_pays_columbarium", "challenge", "family", "ambiguous",
     "母亲墓碑上多刻了一个名字，石匠说图纸是“家里人”亲自送来的。",
     ["墓碑", "石匠"], ["闹鬼", "殉情"], 10, 4, 6, "30-50"),
    ("retouch_the_dead", "challenge", "family", "hard",
     "修图师接到订单：把全家福里已故的老人“修得开心一点”，附言笔迹属于老人本人。",
     ["全家福", "修图"], ["AI复活", "鬼魂"], 10, 4, 5, "25-45"),
    # urban_romance
    ("lease_appendix_monthly", "challenge", "urban_romance", "hard",
     "房东突然搬进来“监督房屋”，租约里那条没人读过的附则正逐月生效。",
     ["租约", "附则"], ["房东是豪门", "囚禁"], 12, 4, 5, "22-35"),
    ("letters_from_last_owner", "challenge", "urban_romance", "ambiguous",
     "过户当天她在信箱发现前业主按月留信，而最后一个月的信还没到。",
     ["信箱", "过户"], ["前任是房东本人", "凶宅"], 10, 4, 5, "22-40"),
    # revenge
    ("silent_partner_one_clause", "challenge", "revenge", "hard",
     "公司濒危时神秘资方入场，条款只有一条：创始人即刻让位。",
     ["注资协议", "创始人"], ["黑道追杀", "商业帝国爽文"], 12, 5, 7, "25-50"),
    ("wedding_gift_two_decades", "challenge", "revenge", "ambiguous",
     "婚礼请柬上多出一位没人敢提的“故人”，随礼是一盒二十年前的喜糖。",
     ["请柬", "喜糖"], ["婚宴血案", "假死归来"], 10, 4, 7, "25-50"),
    # suspense
    ("shift_log_handwriting", "challenge", "suspense", "hard",
     "护工交接班签字本上，有一个签名越写越像病人自己的笔迹。",
     ["签字本", "笔迹"], ["附身", "完美犯罪天才"], 12, 4, 6, "25-50"),
    ("lights_on_dead_circuit", "challenge", "suspense", "ambiguous",
     "老宅各屋的灯每晚按固定顺序亮起，而总闸三年前就断了。",
     ["总闸", "老宅"], ["闹鬼", "盗电贼"], 10, 4, 6, "25-45"),
    # workplace
    ("clean_background_twice", "challenge", "workplace", "hard",
     "新员工的背调干净得像重新生成过，HR按下录用键前想起自己正是当年经手人。",
     ["背调报告", "录用通知"], ["身份窃贼连环案", "老板私生子"], 10, 4, 6, "25-45"),
    # rural
    ("broadcast_changed_words", "challenge", "rural", "hard",
     "村广播站的每日问候三十年没换过一个字，直到今早换了，全村都放下了碗。",
     ["广播", "问候语"], ["空村灵异", "特大灾害"], 10, 4, 7, "30-50"),
]


def load_existing_ids() -> set[str]:
    ids: set[str] = set()
    for split in ("dev", "train", "validation", "challenge"):
        path = CASES / split / "cases.jsonl"
        if path.exists():
            ids.update(
                json.loads(line)["case_id"]
                for line in path.read_text(encoding="utf-8").splitlines()
                if line.strip()
            )
    return ids


def build_records() -> list[dict[str, Any]]:
    existing = load_existing_ids()
    counters = {genre: number for genre, number in EXISTING_IDS.items()}
    license_counter = 30
    records = []
    for (
        family, split, genre, difficulty, prompt,
        required, forbidden, episodes, locations, cast, audience,
    ) in NEW_CASES:
        counters[genre] += 1
        license_counter += 1
        case_id = f"{genre}_{counters[genre]:03d}"
        if case_id in existing:
            raise SystemExit(f"case id collision: {case_id}")
        records.append(
            {
                "schema": "eval-case/v1",
                "case_id": case_id,
                "split": split,
                "premise_family": family,
                "genre": genre,
                "difficulty": difficulty,
                "hard_slice": None,
                "input": prompt,
                "constraints": {
                    "episodes": episodes,
                    "minutes_per_episode": 2,
                    "audience": audience,
                    "rating": "general",
                    "production_level": "low_budget",
                    "max_locations": locations,
                    "max_speaking_cast": cast,
                },
                "required_elements": required,
                "required_conditions": [],
                "forbidden_elements": forbidden,
                "rights": {
                    "source": "internal_authored",
                    "license_id": f"internal-eval-{license_counter:04d}",
                    "allowed_uses": list(SPLIT_USES[split]),
                    "expires_at": None,
                },
            }
        )
    return records


def validate_plan(records: list[dict[str, Any]]) -> None:
    if len(records) != 90:
        raise SystemExit(f"expected 90 new cases, authored {len(records)}")
    genre_counts = Counter(record["genre"] for record in records)
    if genre_counts != Counter(GENRE_QUOTAS):
        raise SystemExit(f"genre quota mismatch: {dict(genre_counts)}")
    split_counts = Counter(record["split"] for record in records)
    if split_counts != Counter(SPLIT_ADDITIONS):
        raise SystemExit(f"split addition mismatch: {dict(split_counts)}")
    families: dict[str, set[str]] = {}
    for record in records:
        families.setdefault(record["premise_family"], set()).add(record["split"])
    spread = {f: s for f, s in families.items() if len(s) > 1}
    if spread:
        raise SystemExit(f"new families cross splits: {spread}")
    licences = [r["rights"]["license_id"] for r in records]
    if len(licences) != len(set(licences)):
        raise SystemExit("duplicate license ids among new cases")


def append_records(records: list[dict[str, Any]]) -> None:
    by_split: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        by_split.setdefault(record["split"], []).append(record)
    for split, bucket in by_split.items():
        path = CASES / split / "cases.jsonl"
        existing_lines = path.read_text(encoding="utf-8").splitlines()
        new_lines = [
            json.dumps(record, ensure_ascii=False, separators=(",", ":"))
            for record in bucket
        ]
        path.write_text(
            "\n".join(existing_lines + new_lines) + "\n", encoding="utf-8"
        )
        print(f"{split}: +{len(new_lines)} -> {len(existing_lines) + len(new_lines)}")


def main() -> int:
    if load_existing_ids() and len(load_existing_ids()) != 30:
        raise SystemExit(
            f"expected the 30-case pilot, found {len(load_existing_ids())} cases; "
            "this expander runs exactly once from the pilot"
        )
    records = build_records()
    validate_plan(records)
    append_records(records)
    print(
        f"appended 90 cases; total {30 + 90}; "
        "now extend split_cases.ASSIGNMENT and run split_cases.py"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
