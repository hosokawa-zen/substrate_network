#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{pallet_prelude::*, dispatch::DispatchResult};
    use frame_system::pallet_prelude::*;
    use frame_support::{
        sp_runtime::traits::Hash,
        traits::{ Randomness, Currency, tokens::ExistenceRequirement },
        transactional
    };
    use sp_io::hashing::blake2_128;

    #[cfg(feature = "std")]
    use frame_support::serde::{Deserialize, Serialize};
    use scale_info::TypeInfo;

    // ACTION #1: Write a Struct to hold Kitty information.
    type AccountOf<T> = <T as frame_system::Config>::AccountId;
    type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // Struct for holding Kitty information.
    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    #[codec(mel_bound())]
    pub struct Kitty<T: Config> {
        pub dna: [u8; 16],
        pub price: Option<BalanceOf<T>>,
        pub gender: Gender,
        pub owner: AccountOf<T>,
    }

    // ACTION #2: Enum declaration for Gender.
    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    pub enum Gender {
        Male,
        Female,
    }

    // The struct on which we build all of our Pallet logic.
    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // TODO: Update the `config` block below
    #[pallet::config]
    pub trait Config: frame_system::Config {
      /// Because this pallet emits events, it depends on the runtime's definition of an event.
      type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
      /// The Currency handler for the Kitties pallet.
      type Currency: Currency<Self::AccountId>;

      type KittyRandomness: Randomness<Self::Hash, Self::BlockNumber>;
      #[pallet::constant]
      type MaxKittyOwned: Get<u32>;
    }

    // TODO: Update the `event` block below
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
      /// A new Kitty was successfully created. \[sender, kitty_id\]
      Created(T::AccountId, T::Hash),
      /// Kitty price was successfully set. \[sender, kitty_id, new_price\]
      PriceSet(T::AccountId, T::Hash, Option<BalanceOf<T>>),
      /// A Kitty was successfully transferred. \[from, to, kitty_id\]
      Transferred(T::AccountId, T::AccountId, T::Hash),
      /// A Kitty was successfully bought. \[buyer, seller, kitty_id, bid_price\]
      Bought(T::AccountId, T::AccountId, T::Hash, BalanceOf<T>),
    }

    // TODO: Update the `error` block below
    #[pallet::error]
    pub enum Error<T> {
      /// Handles arithmetic overflow when incrementing the Kitty counter.
      CountForKittiesOverflow,
      /// An account cannot own more Kitties than `MaxKittyCount`.
      ExceedMaxKittyOwned,
      /// Buyer cannot be the owner.
      BuyerIsKittyOwner,
      /// Cannot transfer a kitty to its owner.
      TransferToSelf,
      /// This kitty already exists
      KittyExists,
      /// This kitty doesn't exist
      KittyNotExist,
      /// Handles checking that the Kitty is owned by the account transferring, buying or setting a price for it.
      NotKittyOwner,
      /// Ensures the Kitty is for sale.
      KittyNotForSale,
      /// Ensures that the buying price is greater than the asking price.
      KittyBidPriceTooLow,
      /// Ensures that an account has enough funds to purchase a Kitty.
      NotEnoughBalance,
    }

      // TODO: add #[pallet::storage] block
    #[pallet::storage]
    #[pallet::getter(fn count_for_kitties)]
    /// Keeps track of the number of Kitties in existence.
    pub(super) type CountForKitties<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn kitties)]
    pub(super) type Kitties<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::Hash,
        Kitty<T>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn kitties_owned)]
    /// Keeps track of what accounts own what Kitty.
    pub(super) type KittiesOwned<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::AccountId,
        BoundedVec<T::Hash, T::MaxKittyOwned>,
        ValueQuery,
    >;

    // TODO: Update the `call` block below
    #[pallet::call]
    impl<T: Config> Pallet<T> {

      // Part III: create_kitty
      #[pallet::weight(100)]
      pub fn create_kitty(origin: OriginFor<T>) -> DispatchResult {
          let sender = ensure_signed(origin)?; 
          let kitty_id = Self::mint(&sender, None, None)?; 
          // Logging to the console
          log::info!("A kitty is born with ID: {:?}.", kitty_id); 

          // ACTION #4: Deposit `Created` event
          Self::deposit_event(Event::Created(sender, kitty_id));
          Ok(())
      }

      // Part IV: set_price
      #[pallet::weight(100)]
      pub fn set_price(origin: OriginFor<T>, kitty_id: T::Hash, new_price: Option<BalanceOf<T>>) -> DispatchResult {
        let sender = ensure_signed(origin)?;
        ensure!(Self::is_kitty_owner(&kitty_id, &sender)?, <Error<T>>::NotKittyOwner);


        let mut kitty = Self::kitties(&kitty_id).ok_or(<Error<T>>::KittyNotExist)?;

        kitty.price = new_price.clone();
        <Kitties<T>>::insert(&kitty_id, kitty);

        Self::deposit_event(Event::PriceSet(sender, kitty_id, new_price));

        Ok(())
      }

      // TODO Part IV: transfer
      #[pallet::weight(100)]
      pub fn transfer(
          origin: OriginFor<T>,
          to: T::AccountId,
          kitty_id: T::Hash
      ) -> DispatchResult {
          let from = ensure_signed(origin)?;

          // Ensure the kitty exists and is called by the kitty owner
          ensure!(Self::is_kitty_owner(&kitty_id, &from)?, <Error<T>>::NotKittyOwner);

          // Verify the kitty is not transferring back to its owner.
          ensure!(from != to, <Error<T>>::TransferToSelf);

          // Verify the recipient has the capacity to receive one more kitty
          let to_owned = <KittiesOwned<T>>::get(&to);
          ensure!((to_owned.len() as u32) < T::MaxKittyOwned::get(), <Error<T>>::ExceedMaxKittyOwned);

          Self::transfer_kitty_to(&kitty_id, &to)?;

          Self::deposit_event(Event::Transferred(from, to, kitty_id));

          Ok(())
      }

      // TODO Part IV: buy_kitty
      #[pallet::weight(100)]
      pub fn buy_kitty(
          origin: OriginFor<T>,
          kitty_id: T::Hash,
          bid_price: BalanceOf<T>
      ) -> DispatchResult {
        let buyer = ensure_signed(origin)?;
        let kitty = Self::kitties(&kitty_id).ok_or(<Error<T>>::KittyNotExist)?;

        if let Some(ask_price) = kitty.price {
          ensure!(ask_price <= bid_price, <Error<T>>::KittyBidPriceTooLow);
        } else {
          Err(<Error<T>>::KittyNotForSale)?;
        }

        // Check the buyer has enough free balance
        ensure!(T::Currency::free_balance(&buyer) >= bid_price, <Error<T>>::NotEnoughBalance);

        // Verify the buyer has the capacity to receive one more kitty
        let to_owned = <KittiesOwned<T>>::get(&buyer);
        ensure!((to_owned.len() as u32) < T::MaxKittyOwned::get(), <Error<T>>::ExceedMaxKittyOwned);

        let seller = kitty.owner.clone();

        // Transfer the amount from buyer to seller
        T::Currency::transfer(&buyer, &seller, bid_price, ExistenceRequirement::KeepAlive)?;

        // Transfer the kitty from seller to buyer
        Self::transfer_kitty_to(&kitty_id, &buyer)?;

        // Deposit relevant Event
        Self::deposit_event(Event::Bought(buyer, seller, kitty_id, bid_price));

        Ok(())
      }

      // TODO Part IV: breed_kitty
      // #[pallet::weight(100)]
      // pub fn breed_kitty(
      //   origin: OriginFor<T>,
      //   parent1: T::Hash,
      //   parent2: T::Hash
      // ) -> DispatchResult {
      //   let sender = ensure_signed(origin)?;
      //   let new_dna = Self::breed_dna(&parent1, &parent2)?;

      //   Self::mint(&sender, Some(new_dna), None)?;

      //   Ok(())
      // }
    }

    impl<T: Config> Pallet<T> {
      // ACTION #4: helper function for Kitty struct

      // TODO Part III: helper functions for dispatchable functions

      // ACTION #6: function to randomly generate DNA

      fn gen_gender() -> Gender {
        let random = T::KittyRandomness::random(&b"gender"[..]).0;
        match random.as_ref()[0] % 2 {
            0 => Gender::Male,
            _ => Gender::Female,
        }
      }
  
      fn gen_dna() -> [u8; 16] {
        let payload = (
            T::KittyRandomness::random(&b"dna"[..]).0,
            <frame_system::Pallet<T>>::extrinsic_index().unwrap_or_default(),
            <frame_system::Pallet<T>>::block_number(),
        );
        payload.using_encoded(blake2_128)
      }
  
      // Helper to mint a Kitty.
      pub fn mint(
        owner: &T::AccountId,
        dna: Option<[u8; 16]>,
        gender: Option<Gender>,
      ) -> Result<T::Hash, Error<T>> {
        let kitty = Kitty::<T> {
            dna: dna.unwrap_or_else(Self::gen_dna),
            price: None,
            gender: gender.unwrap_or_else(Self::gen_gender),
            owner: owner.clone(),
        };

        let kitty_id = T::Hashing::hash_of(&kitty);

        // Performs this operation first as it may fail
        let new_cnt = Self::count_for_kitties().checked_add(1)
            .ok_or(<Error<T>>::CountForKittiesOverflow)?;

        // Check if the kitty does not already exist in our storage map
        ensure!(Self::kitties(&kitty_id) == None, <Error<T>>::KittyExists);

        // Performs this operation first because as it may fail
        <KittiesOwned<T>>::try_mutate(&owner, |kitty_vec| {
            kitty_vec.try_push(kitty_id)
        }).map_err(|_| <Error<T>>::ExceedMaxKittyOwned)?;

        <Kitties<T>>::insert(kitty_id, kitty);
        <CountForKitties<T>>::put(new_cnt);
        Ok(kitty_id)
      }      

      // TODO Part IV: transfer_kitty_to
      #[transactional]
      pub fn transfer_kitty_to(
          kitty_id: &T::Hash,
          to: &T::AccountId,
      ) -> Result<(), Error<T>> {
          let mut kitty = Self::kitties(&kitty_id).ok_or(<Error<T>>::KittyNotExist)?;

          let prev_owner = kitty.owner.clone();

          // Remove `kitty_id` from the KittiesOwned vector of `prev_owner`
          <KittiesOwned<T>>::try_mutate(&prev_owner, |owned| {
              if let Some(ind) = owned.iter().position(|&id| id == *kitty_id) {
                  owned.swap_remove(ind);
                  return Ok(());
              }
              Err(())
          }).map_err(|_| <Error<T>>::KittyNotExist)?;

          // Update the kitty owner
          kitty.owner = to.clone();
          // Reset the ask price so the kitty is not for sale until `set_price()` is called
          // by the current owner.
          kitty.price = None;

          <Kitties<T>>::insert(kitty_id, kitty);

          <KittiesOwned<T>>::try_mutate(to, |vec| {
              vec.try_push(*kitty_id)
          }).map_err(|_| <Error<T>>::ExceedMaxKittyOwned)?;

          Ok(())
      }

      pub fn is_kitty_owner(kitty_id: &T::Hash, acct: &T::AccountId) -> Result<bool, Error<T>> {
        match Self::kitties(kitty_id) {
            Some(kitty) => Ok(kitty.owner == *acct),
            None => Err(<Error<T>>::KittyNotExist)
        }
      }
    }
}