#![feature(default_free_fn)]
use druid::{widget::Label, AppLauncher, Color, PlatformError, Widget, WindowDesc};

use wood::{dewoodify, woods, Dewoodable, Dewooder, Wood, WoodError, Woodable, Wooder};

use chrono::{DateTime, Datelike, Timelike, Utc};
use std::{
    collections::{BTreeMap, HashMap},
    default::default,
    error::Error,
};
use blake3;

use bimap::BiMap;
use priority_queue::PriorityQueue;
use smallvec::SmallVec;

fn build_ui() -> impl Widget<()> {
    Label::new("Hello world")
}

type OID = u128;

struct Base64Bi;
impl Wooder<OID> for Base64Bi {
    fn woodify(&self, v: &OID) -> Wood {
        base64::encode_config(&v.to_le_bytes(), base64::STANDARD_NO_PAD).into()
    }
}
impl Dewooder<OID> for Base64Bi {
    fn dewoodify(&self, v: &Wood) -> Result<OID, Box<WoodError>> {
        let mut ret = [0; 16];
        //TODO test length or else this can be made to panic
        match base64::decode_config_slice(v.initial_str(), base64::STANDARD_NO_PAD, &mut ret) {
            Ok(_o) => Ok(u128::from_le_bytes(ret)),
            Err(e) => Err(Box::new(WoodError::new_with_cause(
                v,
                "couldn't parse the Base64 data".into(),
                Box::new(e),
            ))),
        }
    }
}

//object-safe type erased:
trait DynObject: Woodable {
    fn id(&self) -> OID;
    /// The first version under the most recent branching. May be the same as id() if this is the first version in its branch.
    /// An edit should be a branch when both editing communities should accept that the two edits have become different things.
    /// When you grab the most recent version of an object, you are actually getting the most recent non-branch edit of its branch_root. You wont get edits from other branches, even if they're more recent.
    fn branch_root(&self) -> OID;
    /// The OID of the thing this object is an edit to.
    /// It's a SmallVec because there's usually just one, or if it's a root object, there are none. Sometimes, though, an edit will merge multiple branches or acknowledge multiple previous edits.
    fn editing_prior(&self) -> SmallVec<[OID; 1]>;
    // fn render_big(&self)-> View;
    // fn render_small(&self)-> View;
    // fn render_editable(&self)-> View;
}
//there should be an object that's just LatestVersion(OID), which then then renders as knowledge.latest_version(self.0.branch_root(), approved_by:criterion), where latest_version gets the edit of the branch_root that was approved by group most recently, and displays as that, and also shows an indicator when there's a new version.
//create_version(oid)
//tag_version(oid, tag)
// group:
//     reachable from:person in:number
//     eigenkarma?
// criterion:
//     vote_prediction predictors:group regarding:point_type timeout:number sample_min:number above:number
//     tag of:tag
//     graph_top round:int of:tag
// inclusion group criterion

trait Object: DynObject {
    fn type_provider_name() -> &'static str;
    fn dewoodify_with_id(w: &Wood, id: OID) -> Result<Self, Box<WoodError>>;
}

impl<O: Object> Dewoodable for O {
    fn dewoodify(w: &Wood) -> Result<O, Box<WoodError>>
    where
        O: Sized,
    {
        O::dewoodify_with_id(w, Base64Bi.dewoodify(w.find_val("id")?)?)
    }
}

const icon_spec_colors_against_dark: [Color; 25] = [
    Color::rgb8(0xb5, 0x58, 0x68),
    Color::rgb8(0xb5, 0x58, 0x58),
    Color::rgb8(0xb6, 0x67, 0x57),
    Color::rgb8(0xb7, 0x7d, 0x55),
    Color::rgb8(0xb8, 0x91, 0x54),
    Color::rgb8(0xb7, 0x9a, 0x55),
    Color::rgb8(0xb7, 0xa7, 0x55),
    Color::rgb8(0xb7, 0xb7, 0x55),
    Color::rgb8(0x9b, 0xb7, 0x55),
    Color::rgb8(0x7d, 0xb7, 0x55),
    Color::rgb8(0x57, 0xc0, 0x6b),
    Color::rgb8(0x54, 0xb8, 0x8a),
    Color::rgb8(0x58, 0xb5, 0x9b),
    Color::rgb8(0x58, 0xb5, 0xaf),
    Color::rgb8(0x5c, 0x9f, 0xbf),
    Color::rgb8(0x59, 0x98, 0xd4),
    Color::rgb8(0x56, 0x7e, 0xd0),
    Color::rgb8(0x5a, 0x6e, 0xdd),
    Color::rgb8(0x60, 0x61, 0xdf),
    Color::rgb8(0x7f, 0x65, 0xc2),
    Color::rgb8(0x96, 0x62, 0xc0),
    Color::rgb8(0xac, 0x5e, 0xc9),
    Color::rgb8(0xbe, 0x62, 0xc1),
    Color::rgb8(0xb5, 0x58, 0x96),
    Color::rgb8(0xb5, 0x58, 0x7c),
];
const color_is_mistakable_for_next_color: [bool; 25] = [
    false, true, true, false, true, true, true, false, false, true, false, true, true, false, true,
    false, true, true, false, true, true, true, false, true, true,
];
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct IconSpec {
    angle: u8, //0 starts horizontal, color at top, rotating at 45° increments
    color: u8, //indexes of the above
    shade: u8, //darkest, dim, lighter, light
}
impl IconSpec {
    fn new(v: u32) -> Result<IconSpec, Box<dyn Error>> {
        let ret = IconSpec {
            angle: v.to_le_bytes()[0],
            color: v.to_le_bytes()[1],
            shade: v.to_le_bytes()[2],
        };
        // TODO: Do a string error
        // if ret.angle > 7 || ret.color > 24 || shade > 4 { return Err() }
        Ok(ret)
    }
    fn color(&self) -> Color {
        icon_spec_colors_against_dark[self.color as usize]
    }
    /// used to check for impersonation?
    fn similar(&self, other: &IconSpec) -> bool {
        // angles are close
        ((self.angle + 1)%8 == other.angle || (other.angle + 1)%8 == self.angle) &&
        // and colors are close
        {
            let max = self.color.max(other.color);
            let min = self.color.min(other.color);
            min == max ||
            (
                (min + 1)%color_is_mistakable_for_next_color.len() == max &&
                color_is_mistakable_for_next_color[min as usize]
            )
        }
    }
}

struct Profile {
    id: OID,
    // icon: IconSpec,
    name: String,
    description: String,
}
impl Woodable for Profile {
    fn woodify(&self) -> Wood {
        woods![
            self.type_provider_name(),
            woods!["id", Base64Bi.woodify(&self.id)],
            woods!["name", self.name.woodify()],
            woods!["description", self.description.woodify()],
        ]
    }
}
impl Object for Profile {
    fn type_provider_name() -> &'static str {
        "profile"
    }
    fn dewoodify_with_id(v: &Wood, id: OID) -> Result<Self, Box<WoodError>>
    where
        Self: Sized,
    {
        let mut tm = v.find("name")?.tail();
        let mut name = match tm.next() {
            Some(w) => w.initial_str().as_string(),
            None => "".into(),
        };
        for w in tm {
            name.push(NAME_SEPARATOR);
            name.push(w.initial_str());
        }
        Ok(Profile {
            id,
            name,
            description: dewoodify(v.find_val("description")?)?,
        })
    }
}
impl Objecto for Profile {
    fn id(&self) -> OID {
        self.id
    }
}

const NAME_SEPARATOR: char = ' '; //EM Quad

struct ObKnow {
    date_seen: DateTime<Utc>,
    ob: Box<dyn Object>,
    /// where it is in the expiry queue
    queue_place: u32,
    /// lower priority gets expired more often
    priority: i8,
}
struct Knowledge {
    ob_cache: HashMap<OID, ObKnow>,
    expiry_queues: Vec<PriorityQueue<DateTime<Utc>, OID>>,
    names_to_oids: BTreeMap<String, SmallVec<[OID; 1]>>,
    oids_to_names: HashMap<String, Vec<OID>>,
    user_short_names: HashMap<OID, String>,
    max_known_id: OID,
}
impl Knowledge {
    fn gen_id(&mut self) -> OID {
        self.max_known_id += 1;
        self.max_known_id
    }
}

macro_rules! grab_id_if_can {
    ($db:ident, $rw:ident, $to_gen_ids_for:ident, $max_known_id:ident, $b:expr) => {
        let lamba = $b;
        if let Some(idw) = $rw.seek_val("id") {
            let id = dewoodify(idw)?;
            $max_known_id = $max_known_id.max(id);
            lamba(&mut $db, id);
        } else {
            $to_gen_ids_for.push(lamba);
        }
    };
}

fn obweb_hash_wood(w: &Wood) -> OID {
    let mut hasher = blake3::Hasher::new();
    s
}

type Res<T> = Resul<T, Box<dyn Error>>;

fn add_provider<O: Object>(
    providers: &mut HashMap<&'static str, fn(db: &mut Knowledge, oid: OID, w: &Wood) -> Res<()>>,
) {
    providers.push(O::type_provider_name(), |db, oid, w| {
        db.consider(oid, Box::new(O::dewoodify(w)?));
        Ok(())
    });
}

fn init_db() -> Result<Knowledge, Box<dyn Error>> {
    let initw = wood::parse_multiline_termpose(std::fs::read_to_string("db_init.term")?.as_str())?;

    let mut knows = Knowledge {
        max_known_id: 1,
        ..default()
    };

    //first makes the ones with stated ids, then generates ids for the rest

    let mut to_gen_ids_for: Vec<
        Box<dyn for<'r> FnOnce(&'r mut Knowledge, OID) -> Result<(), Box<dyn Error>>>,
    > = Vec::new();
    let mut max_known_id = 0u128;

    let mut providers = HashMap::new();
    let p = &mut providers;
    add_provider::<Profile>(p);
    add_provider::<Endorsement>(p);
    add_provider::<Post>(p);

    for rw in initw.contents() {
        match rw.initial_str() {
            "insert" => {
                let whether_reporting = rw.head().seek("report_ids").is_some();
                for io in rw.tail() {
                    if let Some(f) = providers.get(io.initial_str()) {
                        let oid = obweb_hash_wood(w);
                        if whether_reporting {
                            println!("{}: {}", oid, io.initial_str());
                        }
                        f(&mut knows, oid, io);
                    } else {
                        println!(
                            "warning: unrecognized init term, {}",
                            wood::to_woodslist(rw)
                        );
                    }
                }
            }
            "make_reply_tree" => {
                println!("not yet ready to build replies (you don't even have the ID yet)");
                // let replying_to: AID = Base64Bi.dewoodify(rw.find_val("to")?)?;
                // let mut messages = rw.tail();
                // rw.next();
                // for messagew in messages {
                //     messagew.initial_str()
                // }
            }
            _ => {
                    println!("warning: unrecognized init term");
            }
        }
    }
    for b in to_gen_ids_for {
        max_known_id += 1;
        b(&mut knows, max_known_id)?;
    }

    Ok(knows)
}

fn main() -> Result<(), PlatformError> {
    AppLauncher::with_window(WindowDesc::new(build_ui)).launch(())?;
    Ok(())
}
