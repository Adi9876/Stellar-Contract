#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, vec, Address, Env, IntoVal, Map, Symbol,
    Timepoint, Vec, I256,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentLink {
    merchant: Address,
    amount: I256,
    active: bool,
    description: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionPlan {
    merchant: Address,
    amount: I256,
    interval: u32,
    active: bool,
    name: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    subscriber: Address,
    plan_id: u32,
    start_time: Timepoint,
    last_payment: Timepoint,
    active: bool,
}

// Storage Keys (all <=9 chars)
const OWNER: Symbol = symbol_short!("OWNER");
const TOKEN: Symbol = symbol_short!("TOKEN");
const MERCH: Symbol = symbol_short!("MERCH");
const LCTR: Symbol = symbol_short!("LCTR");
const PCTR: Symbol = symbol_short!("PCTR");
const SCTR: Symbol = symbol_short!("SCTR");
const PLINK: Symbol = symbol_short!("PLINK");
const SPLAN: Symbol = symbol_short!("SPLAN");
const SUBS: Symbol = symbol_short!("SUBS");

#[contract]
pub struct PaymentGateway;

#[contractimpl]
impl PaymentGateway {
    pub fn init(env: Env, invoker: Address, token: Address) {
        invoker.require_auth();
        env.storage().instance().set(&OWNER, &invoker);
        env.storage().instance().set(&TOKEN, &token);
        env.storage()
            .instance()
            .set(&MERCH, &Vec::<Address>::new(&env));
        env.storage().instance().set(&LCTR, &0u32);
        env.storage().instance().set(&PCTR, &0u32);
        env.storage().instance().set(&SCTR, &0u32);
    }

    fn only_owner(env: &Env, invoker: &Address) {
        let o: Address = env.storage().instance().get(&OWNER).expect("OWNER not set");
        invoker.require_auth();
        assert!(invoker == &o, "only owner");
    }

    pub fn add_merchant(env: Env, invoker: Address, merchant: Address) {
        Self::only_owner(&env, &invoker);
        let mut merchants: Vec<Address> = env
            .storage()
            .instance()
            .get(&MERCH)
            .unwrap_or(Vec::new(&env));
        assert!(!merchants.contains(&merchant), "already authorized");
        merchants.push_back(merchant.clone());
        env.storage().instance().set(&MERCH, &merchants);
        env.events().publish((symbol_short!("MAdd"),), &merchant);
    }

    pub fn remove_merchant(env: Env, invoker: Address, merchant: Address) {
        Self::only_owner(&env, &invoker);
        let mut merchants: Vec<Address> = env
            .storage()
            .instance()
            .get(&MERCH)
            .unwrap_or(Vec::new(&env));
        assert!(merchants.contains(&merchant), "not authorized");
        // Remove merchant (no retain, manual loop)
        let mut new_merchants = Vec::new(&env);
        for i in 0..merchants.len() {
            let m = merchants.get_unchecked(i);
            if m != merchant {
                new_merchants.push_back(m);
            }
        }
        env.storage().instance().set(&MERCH, &new_merchants);
        env.events().publish((symbol_short!("MRem"),), &merchant);
    }

    fn is_merchant(env: &Env, who: &Address) -> bool {
        let merchants: Vec<Address> = env
            .storage()
            .instance()
            .get(&MERCH)
            .unwrap_or(Vec::new(&env));
        merchants.contains(who)
    }

    pub fn create_payment_link(env: Env, invoker: Address, amount: I256, description: Symbol) {
        invoker.require_auth();
        assert!(Self::is_merchant(&env, &invoker), "not authorized");
        assert!(amount > I256::from_i128(&env, 0), "amount>0");
        let mut ctr: u32 = env.storage().instance().get(&LCTR).unwrap_or(0);
        ctr += 1;
        env.storage().instance().set(&LCTR, &ctr);
        let pl = PaymentLink {
            merchant: invoker.clone(),
            amount: amount.clone(),
            active: true,
            description: description.clone(),
        };
        let mut links: Map<u32, PaymentLink> = env
            .storage()
            .instance()
            .get(&PLINK)
            .unwrap_or(Map::new(&env));
        links.set(ctr, pl);
        env.storage().instance().set(&PLINK, &links);
        env.events().publish((symbol_short!("PLCr"), ctr), &ctr);
    }

    pub fn process_payment(env: Env, invoker: Address, link_id: u32) {
        invoker.require_auth();
        let mut links: Map<u32, PaymentLink> = env
            .storage()
            .instance()
            .get(&PLINK)
            .unwrap_or(Map::new(&env));
        let mut link = links.get(link_id).expect("link not found");
        assert!(link.active, "inactive link");
        let payer = invoker;
        let token: Address = env.storage().instance().get(&TOKEN).expect("Token");
        env.invoke_contract::<()>(
            &token,
            &symbol_short!("trf_from"),
            vec![
                payer.clone().into_val(&env),
                link.merchant.clone().into_val(&env),
                link.amount.clone().into_val(&env),
            ],
        );
        env.events()
            .publish((symbol_short!("Payd"), link_id), &link_id);
    }

    pub fn create_subscription_plan(
        env: Env,
        invoker: Address,
        amount: I256,
        interval: u32,
        name: Symbol,
    ) {
        invoker.require_auth();
        assert!(Self::is_merchant(&env, &invoker), "not authorized");
        assert!(amount > I256::from_i128(&env, 0), "amount>0");
        assert!(interval > 0, "interval>0");
        let mut ctr: u32 = env.storage().instance().get(&PCTR).unwrap_or(0);
        ctr += 1;
        env.storage().instance().set(&PCTR, &ctr);
        let sp = SubscriptionPlan {
            merchant: invoker.clone(),
            amount: amount.clone(),
            interval,
            active: true,
            name: name.clone(),
        };
        let mut plans: Map<u32, SubscriptionPlan> = env
            .storage()
            .instance()
            .get(&SPLAN)
            .unwrap_or(Map::new(&env));
        plans.set(ctr, sp);
        env.storage().instance().set(&SPLAN, &plans);
        env.events().publish((symbol_short!("SPCr"), ctr), &ctr);
    }

    pub fn subscribe(env: Env, invoker: Address, plan_id: u32) {
        invoker.require_auth();
        let mut plans: Map<u32, SubscriptionPlan> = env
            .storage()
            .instance()
            .get(&SPLAN)
            .unwrap_or(Map::new(&env));
        let plan = plans.get(plan_id).expect("plan not found");
        assert!(plan.active, "plan not active");
        let subber = invoker.clone();
        let now = Timepoint::from_unix(&env, env.ledger().timestamp());
        let mut ctr: u32 = env.storage().instance().get(&SCTR).unwrap_or(0);
        ctr += 1;
        env.storage().instance().set(&SCTR, &ctr);
        let sub = Subscription {
            subscriber: subber.clone(),
            plan_id,
            start_time: now,
            last_payment: now,
            active: true,
        };
        let mut subs: Map<(Address, u32), Subscription> = env
            .storage()
            .instance()
            .get(&SUBS)
            .unwrap_or(Map::new(&env));
        subs.set((subber.clone(), ctr), sub);
        env.storage().instance().set(&SUBS, &subs);
        let token: Address = env.storage().instance().get(&TOKEN).expect("Token");
        env.invoke_contract::<()>(
            &token,
            &symbol_short!("trf_from"),
            vec![
                subber.clone().into_val(&env),
                plan.merchant.clone().into_val(&env),
                plan.amount.clone().into_val(&env),
            ],
        );
        env.events().publish((symbol_short!("Subd"), ctr), &ctr);
        env.events().publish((symbol_short!("SPay"), ctr), &ctr);
    }

    pub fn process_subscription_payment(
        env: Env,
        invoker: Address,
        subscriber: Address,
        subscription_id: u32,
    ) {
        invoker.require_auth();
        let mut subs: Map<(Address, u32), Subscription> = env
            .storage()
            .instance()
            .get(&SUBS)
            .unwrap_or(Map::new(&env));
        let mut sub = subs
            .get((subscriber.clone(), subscription_id))
            .expect("subscription not found");
        assert!(sub.active, "sub inactive");
        let plans: Map<u32, SubscriptionPlan> = env
            .storage()
            .instance()
            .get(&SPLAN)
            .unwrap_or(Map::new(&env));
        let plan = plans.get(sub.plan_id).expect("plan not found");
        assert!(plan.active, "plan inactive");
        let now = Timepoint::from_unix(&env, env.ledger().timestamp());
        let next_due =
            Timepoint::from_unix(&env, sub.last_payment.to_unix() + (plan.interval as u64));
        assert!(now.to_unix() >= next_due.to_unix(), "not due");
        let token: Address = env.storage().instance().get(&TOKEN).expect("Token");
        env.invoke_contract::<()>(
            &token,
            &symbol_short!("trf_from"),
            vec![
                subscriber.clone().into_val(&env),
                plan.merchant.clone().into_val(&env),
                plan.amount.clone().into_val(&env),
            ],
        );
        sub.last_payment = now;
        subs.set((subscriber.clone(), subscription_id), sub.clone());
        env.storage().instance().set(&SUBS, &subs);
        env.events()
            .publish((symbol_short!("SPay"), subscription_id), &subscription_id);
    }

    pub fn cancel_subscription(env: Env, invoker: Address, subscription_id: u32) {
        invoker.require_auth();
        let subber = invoker.clone();
        let mut subs: Map<(Address, u32), Subscription> = env
            .storage()
            .instance()
            .get(&SUBS)
            .unwrap_or(Map::new(&env));
        let mut sub = subs.get((subber.clone(), subscription_id)).expect("no sub");
        assert!(sub.active, "already inactive");
        assert!(
            sub.subscriber == subber.clone() || Self::is_merchant(&env, &invoker),
            "not authorized"
        );
        sub.active = false;
        subs.set((subber.clone(), subscription_id), sub.clone());
        env.storage().instance().set(&SUBS, &subs);
        env.events()
            .publish((symbol_short!("SCnl"), subscription_id), &subscription_id);
    }

    pub fn deactivate_payment_link(env: Env, invoker: Address, link_id: u32) {
        invoker.require_auth();
        let m = invoker;
        let mut links: Map<u32, PaymentLink> = env
            .storage()
            .instance()
            .get(&PLINK)
            .unwrap_or(Map::new(&env));
        let mut link = links.get(link_id).expect("no link");
        assert!(link.merchant == m, "not merchant");
        assert!(link.active, "already inactive");
        link.active = false;
        links.set(link_id, link);
        env.storage().instance().set(&PLINK, &links);
    }

    pub fn deactivate_subscription_plan(env: Env, invoker: Address, plan_id: u32) {
        invoker.require_auth();
        let m = invoker;
        let mut plans: Map<u32, SubscriptionPlan> = env
            .storage()
            .instance()
            .get(&SPLAN)
            .unwrap_or(Map::new(&env));
        let mut plan = plans.get(plan_id).expect("no plan");
        assert!(plan.merchant == m, "not merchant");
        assert!(plan.active, "already inactive");
        plan.active = false;
        plans.set(plan_id, plan);
        env.storage().instance().set(&SPLAN, &plans);
    }
}
